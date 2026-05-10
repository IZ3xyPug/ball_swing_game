use quartz::*;
use std::sync::{Arc, Mutex};
use crate::constants::*;
use crate::audio_state;
use crate::images::*;
use crate::objects::ui_text_spec;
use crate::shop;

const MENU_TRACKS: [&str; 2] = [ASSET_MENU_BGM, ASSET_MENU_BGM_2];

fn volume_value(c: &Canvas, var: &str, default: f32) -> f32 {
    match c.get_var(var) {
        Some(Value::F32(v)) => v.clamp(0.0, 1.0),
        _ => default,
    }
}

fn set_volume_value(c: &mut Canvas, var: &str, v: f32) {
    c.set_var(var, v.clamp(0.0, 1.0));
}

fn slider_bar(v: f32, steps: usize) -> String {
    let clamped = v.clamp(0.0, 1.0);
    let filled = ((clamped * steps as f32).round() as usize).min(steps);
    format!("{}{}", "#".repeat(filled), "-".repeat(steps - filled))
}

fn menu_music_volume(c: &Canvas, base: f32) -> f32 {
    let master = volume_value(c, "vol_master", 1.0);
    let music = volume_value(c, "vol_music", 1.0);
    (base * master * music).clamp(0.0, 1.0)
}

fn play_menu_track(c: &mut Canvas, idx: usize) {
    let track_idx = idx % MENU_TRACKS.len();
    let handle = c.play_sound_with(
        MENU_TRACKS[track_idx],
        SoundOptions::new().volume(menu_music_volume(c, 0.18)).looping(false),
    );
    audio_state::replace_menu_bgm(handle);
    c.set_var("menu_bgm_track_index", track_idx as i32);
}

/// Menu lives at y = MENU_Y..MENU_Y+VH; shop lives at y = 0..VH.
/// Camera pans between these two regions (from VH down to 0) for the transition.
const MENU_Y: f32 = VH;

/// Cached aurora earth background — decoded and resized once, then Arc-shared.
static AURORA_BG_CACHE: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();

/// Load aurora_earth.gif at the given dimensions using Lanczos3 for high-quality resize.
/// Result is cached so subsequent calls clone an Arc pointer instead of re-decoding.
fn bright_background_2(w: f32, h: f32) -> Image {
    let arc = AURORA_BG_CACHE.get_or_init(|| {
        let aurora_src = image::load_from_memory(include_bytes!("../assets/aurora_earth.gif"))
            .expect("aurora_earth.gif decode failed")
            .to_rgba8();
        let pixels = image::imageops::resize(
            &aurora_src, w as u32, h as u32, image::imageops::FilterType::Lanczos3,
        );
        Arc::new(pixels)
    });
    Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: Arc::clone(arc), color: None }
}

const MENU_UI_ANIM_FRAMES: i32 = 60;

fn tutorial_apply_page(c: &mut Canvas, page: i32) {
    let page = page.clamp(0, 2);
    c.set_var("tutorial_page", page);

    const TITLES: [&str; 3] = [
        "TUTORIAL  1/3",
        "TUTORIAL  2/3",
        "TUTORIAL  3/3",
    ];
    const BODIES: [&str; 3] = [
        "SWING BETWEEN HOOKS TO BUILD SPEED AND DISTANCE.",
        "GRAB POWERUPS TO IMPROVE SURVIVAL AND MOMENTUM.",
        "WATCH FLOOR DANGER, THEN CHAIN MOVES FOR LONG RUNS.",
    ];

    for i in 0..3 {
        if let Some(obj) = c.get_game_object_mut(&format!("tutorial_window_{i}")) {
            obj.visible = i == page;
        }
    }

    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("tutorial_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                TITLES[page as usize],
                &font,
                54.0,
                Color(220, 238, 255, 255),
                1300.0,
            )));
        }

        if let Some(obj) = c.get_game_object_mut("tutorial_body_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                BODIES[page as usize],
                &font,
                28.0,
                Color(200, 218, 236, 235),
                1600.0,
            )));
        }

        if let Some(obj) = c.get_game_object_mut("tutorial_next_text") {
            let label = if page >= 2 { "FINISH" } else { "NEXT" };
            obj.set_drawable(Box::new(ui_text_spec(
                label,
                &font,
                18.0,
                Color(245, 250, 255, 255),
                95.0,
            )));
        }
    }
}

pub fn build_tutorial_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx,
        "tutorial_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH),
        (-400.0, 0.0),
        vec![],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    let bg_tint = GameObject::new_rect(
        ctx,
        "tutorial_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(40, 70, 130, 165))),
        (VW + 800.0, VH),
        (-400.0, 0.0),
        vec![],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    let mut scene = Scene::new("tutorial")
        .with_object("tutorial_bg", bg)
        .with_object("tutorial_bg_tint", bg_tint);

    let win_w = 2960.0;
    let win_h = 1680.0;
    let win_x = VW * 0.5 - win_w * 0.5;
    let win_y = VH * 0.09;

    for i in 0..3 {
        let mut img = image::RgbaImage::new(win_w as u32, win_h as u32);
        for py in 0..img.height() {
            for px in 0..img.width() {
                let border = px < 5 || px >= img.width() - 5 || py < 5 || py >= img.height() - 5;
                let glow = ((py as f32 / img.height() as f32) * 24.0) as u8;
                img.put_pixel(
                    px,
                    py,
                    image::Rgba([
                        if border { 90 + i as u8 * 16 } else { 15 + glow / 4 },
                        if border { 150 + i as u8 * 8 } else { 20 + glow / 3 },
                        if border { 230 } else { 44 + glow / 2 },
                        if border { 255 } else { 235 },
                    ]),
                );
            }
        }
        let mut win = GameObject::new_rect(
            ctx,
            format!("tutorial_window_{i}").into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (win_w, win_h), 0.0),
                image: img.into(),
                color: None,
            }),
            (win_w, win_h),
            (win_x, win_y),
            vec!["ui".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        win.visible = i == 0;
        scene = scene.with_object(format!("tutorial_window_{i}"), win);
    }

    let title_w = 1600.0;
    let body_w = 1600.0;
    let text_shift_x = -1650.0;
    let title_extra_x = 500.0;

    let title_text = GameObject::build("tutorial_title_text")
        .size(title_w, 120.0)
        .position(
            VW * 0.5 - title_w * 0.5 + text_shift_x + title_extra_x,
            win_y + 120.0,
        )
        .tag("ui")
        .build(ctx);
    let body_text = GameObject::build("tutorial_body_text")
        .size(body_w, 120.0)
        .position(VW * 0.5 - body_w * 0.5 + text_shift_x, win_y + 330.0)
        .tag("ui")
        .build(ctx);

    let next_btn_w = 300.0;
    let next_btn_h = 105.0;
    let next_btn_x = win_x + win_w - next_btn_w - 60.0;
    let next_btn_y = win_y + win_h - next_btn_h - 60.0;
    let next_btn = {
        let mut img = image::RgbaImage::new(next_btn_w as u32, next_btn_h as u32);
        for py in 0..img.height() {
            for px in 0..img.width() {
                let border = px < 3 || px >= img.width() - 3 || py < 3 || py >= img.height() - 3;
                img.put_pixel(px, py, image::Rgba([55, if border { 170 } else { 120 }, 85, 235]));
            }
        }
        GameObject::new_rect(
            ctx,
            "tutorial_next_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (next_btn_w, next_btn_h), 0.0),
                image: img.into(),
                color: None,
            }),
            (next_btn_w, next_btn_h),
            (next_btn_x, next_btn_y),
            vec!["ui".into(), "button".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };
    let next_text = GameObject::build("tutorial_next_text")
        .size(next_btn_w, next_btn_h)
        .position(next_btn_x, next_btn_y + 8.0)
        .tag("ui")
        .build(ctx);

    scene
        .with_object("tutorial_title_text", title_text)
        .with_object("tutorial_body_text", body_text)
        .with_object("tutorial_next_btn", next_btn)
        .with_object("tutorial_next_text", next_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom {
                    name: "tutorial_next".into(),
                },
                target: Target::name("tutorial_next_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("tutorial_next_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);
            tutorial_apply_page(canvas, 0);

            canvas.register_custom_event("tutorial_next".into(), |c| {
                let page = c.get_i32("tutorial_page");
                if page >= 2 {
                    c.load_scene("menu");
                } else {
                    tutorial_apply_page(c, page + 1);
                }
            });
        })
}

pub static GAME_MODES: &[(&str, &str)] = &[
    ("FREE ROAM", "SWING FREELY   \u{2022}   SANDBOX MODE"),
];

fn menu_mode_selector_img() -> image::RgbaImage {
    let w = 800u32;
    let h = 140u32;
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h { for px in 0..w {
        img.put_pixel(px, py, image::Rgba([18, 26, 48, 230]));
    }}
    draw_rect(&mut img, 0, 0, w, 3, [90, 170, 255, 255]);
    draw_rect(&mut img, 0, h-3, w, 3, [90, 170, 255, 255]);
    draw_rect(&mut img, 0, 0, 3, h, [90, 170, 255, 255]);
    draw_rect(&mut img, w-3, 0, 3, h, [90, 170, 255, 255]);
    let mid = (h / 2) as i32;
    for i in 0..23i32 {
        let x = (28 + i) as u32;
        for dy in -i..=i {
            let py = (mid + dy) as u32;
            if py < h { img.put_pixel(x, py, image::Rgba([140, 210, 255, 200])); }
        }
    }
    for i in 0..23i32 {
        let x = (w as i32 - 29 - i) as u32;
        for dy in -i..=i {
            let py = (mid + dy) as u32;
            if py < h { img.put_pixel(x, py, image::Rgba([140, 210, 255, 200])); }
        }
    }
    img
}

pub fn build_menu_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "menu_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, MENU_Y), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "menu_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(70, 120, 255, 110))),
        (VW + 800.0, VH), (-400.0, MENU_Y), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let title = {
        let (w, h) = (1700u32, 260u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let t = px as f32 / w as f32;
            img.put_pixel(px, py, image::Rgba([
                (50.0  + 140.0*t) as u8,
                (200.0 +  55.0*t) as u8,
                (255.0 -  80.0*t) as u8,
                255,
            ]));
        }}
        GameObject::new_rect(
            ctx, "menu_title".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, MENU_Y + VH*0.14),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    }; // title

    let menu_sub = {
        let (w, h) = (600u32, 60u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            img.put_pixel(px, py, image::Rgba([40, 60, 100, 180]));
        }}
        draw_rect(&mut img, 0, 0, w, 2, [90, 140, 220, 255]);
        draw_rect(&mut img, 0, h-2, w, 2, [90, 140, 220, 255]);
        GameObject::new_rect(
            ctx, "menu_sub".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, MENU_Y + VH*0.40),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let menu_mode_selector = GameObject::new_rect(
        ctx, "menu_mode_selector".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (800.0, 140.0), 0.0),
            image: menu_mode_selector_img().into(),
            color: None,
        }),
        (800.0, 140.0), (VW/2.0 - 400.0, MENU_Y + VH*0.46),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let start_btn = {
        let (w, h) = (540u32, 130u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([60, if border {200} else {130}, 180, 240]));
        }}
        GameObject::new_rect(
            ctx, "start_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, MENU_Y + VH*0.68),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    // ── Shop button ──────────────────────────────────────────────────────────
    let shop_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([if border {220} else {90}, if border {190} else {60}, 60, 230]));
        }}
        GameObject::new_rect(
            ctx, "menu_shop_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32 - 20.0, MENU_Y + VH*0.81),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    // ── Settings button ───────────────────────────────────────────────────────
    let settings_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([60, if border {200} else {90}, if border {240} else {160}, 230]));
        }}
        GameObject::new_rect(
            ctx, "menu_settings_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 + 20.0, MENU_Y + VH*0.81),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let menu_title_text = GameObject::build("menu_title_text")
        .size(1700.0, 260.0)
        .position(VW * 0.5 - 850.0, MENU_Y + VH * 0.14 + (260.0 - 74.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_sub_text = GameObject::build("menu_sub_text")
        .size(600.0, 60.0)
        .position(VW * 0.5 - 300.0, MENU_Y + VH * 0.40 + (60.0 - 22.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_mode_name_text = GameObject::build("menu_mode_name_text")
        .size(640.0, 140.0)
        .position(VW * 0.5 - 320.0, MENU_Y + VH * 0.46 + (140.0 - 52.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_mode_desc_text = GameObject::build("menu_mode_desc_text")
        .size(800.0, 60.0)
        .position(VW * 0.5 - 400.0, MENU_Y + VH * 0.46 + 152.0)
        .tag("ui")
        .build(ctx);

    let menu_start_text = GameObject::build("menu_start_text")
        .size(540.0, 130.0)
        .position(VW * 0.5 - 270.0, MENU_Y + VH * 0.68 + (130.0 - 24.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_shop_text = GameObject::build("menu_shop_text")
        .size(420.0, 110.0)
        .position(VW / 2.0 - 420.0 - 20.0, MENU_Y + VH * 0.81 + (110.0 - 36.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_settings_text = GameObject::build("menu_settings_text")
        .size(420.0, 110.0)
        .position(VW / 2.0 + 20.0, MENU_Y + VH * 0.81 + (110.0 - 36.0) / 2.0)
        .tag("ui")
        .build(ctx);

    // ── Second row: Achievements / Stats / Daily Reward ──────────────────────
    let achievements_btn = {
        let (w, h) = (280u32, 90u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([if border {200} else {100}, 60, if border {240} else {160}, 230]));
        }}
        GameObject::new_rect(
            ctx, "menu_achievements_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - 440.0, MENU_Y + VH*0.88),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    let stats_btn = {
        let (w, h) = (280u32, 90u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([if border {240} else {160}, if border {180} else {90}, 40, 230]));
        }}
        GameObject::new_rect(
            ctx, "menu_stats_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - 140.0, MENU_Y + VH*0.88),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    let daily_btn = {
        let (w, h) = (280u32, 90u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([40, if border {220} else {120}, if border {180} else {80}, 230]));
        }}
        GameObject::new_rect(
            ctx, "menu_daily_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 + 160.0, MENU_Y + VH*0.88),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let menu_achievements_text = GameObject::build("menu_achievements_text")
        .size(280.0, 90.0)
        .position(VW/2.0 - 440.0, MENU_Y + VH * 0.88 + (90.0 - 30.0) / 2.0)
        .tag("ui")
        .build(ctx);
    let menu_stats_text = GameObject::build("menu_stats_text")
        .size(280.0, 90.0)
        .position(VW/2.0 - 140.0, MENU_Y + VH * 0.88 + (90.0 - 30.0) / 2.0)
        .tag("ui")
        .build(ctx);
    let menu_daily_text = GameObject::build("menu_daily_text")
        .size(280.0, 90.0)
        .position(VW/2.0 + 160.0, MENU_Y + VH * 0.88 + (90.0 - 30.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let scene = Scene::new("menu")
        .with_object("menu_bg",             bg)
        .with_object("menu_bg_tint",        bg_tint)
        .with_object("menu_title",          title)
        .with_object("menu_sub",            menu_sub)
        .with_object("menu_mode_selector",  menu_mode_selector)
        .with_object("start_btn",           start_btn)
        .with_object("menu_shop_btn",       shop_btn)
        .with_object("menu_settings_btn",   settings_btn)
        .with_object("menu_title_text",     menu_title_text)
        .with_object("menu_sub_text",       menu_sub_text)
        .with_object("menu_mode_name_text", menu_mode_name_text)
        .with_object("menu_mode_desc_text", menu_mode_desc_text)
        .with_object("menu_start_text",     menu_start_text)
        .with_object("menu_shop_text",          menu_shop_text)
        .with_object("menu_settings_text",      menu_settings_text)
        .with_object("menu_achievements_btn",    achievements_btn)
        .with_object("menu_stats_btn",           stats_btn)
        .with_object("menu_daily_btn",           daily_btn)
        .with_object("menu_achievements_text",   menu_achievements_text)
        .with_object("menu_stats_text",          menu_stats_text)
        .with_object("menu_daily_text",          menu_daily_text);

    // Embed shop objects at y=0..VH (camera pans from VH to 0 to reveal them)
    shop::extend_with_shop(ctx, scene)
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "goto_game".into() },
                target: Target::name("start_btn"),
            },
            Target::name("start_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_game".into() },
                target: Target::name("start_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("start_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_shop".into() },
                target: Target::name("menu_shop_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_shop_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_menu_settings".into() },
                target: Target::name("menu_settings_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_settings_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_achievements".into() },
                target: Target::name("menu_achievements_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_achievements_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_stats".into() },
                target: Target::name("menu_stats_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_stats_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_daily_reward".into() },
                target: Target::name("menu_daily_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_daily_btn"),
        )
        .on_enter(|canvas| {
            // Returning to main menu is the only transition that stops in-game music.
            audio_state::stop_game_bgm();
            // Menu track is randomized whenever entering menu.
            let random_idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0)) % MENU_TRACKS.len();
            play_menu_track(canvas, random_idx);
            // World is 2×VH tall: shop at y=0..VH, menu at y=VH..2VH.
            // Camera starts pointing at menu (y=MENU_Y). goto_shop lerps y to 0.
            let mut cam = Camera::new((VW, VH * 2.0), (VW, VH));
            cam.position = (0.0, MENU_Y);
            canvas.set_camera(cam);
            canvas.set_var("menu_cam_target_y", MENU_Y);
            canvas.set_var("menu_in_shop", false);

            canvas.set_var("menu_text_dirty", true);

            let selected = Arc::new(Mutex::new(0usize));

            let menu_key_registered = matches!(canvas.get_var("menu_key_registered"), Some(Value::Bool(true)));
            if !menu_key_registered {
                canvas.on_key_press({
                    let sel = Arc::clone(&selected);
                    move |c, key| {
                        if !c.is_scene("menu") { return; }
                        // In shop: route all keystrokes to the carousel handler.
                        if matches!(c.get_var("menu_in_shop"), Some(Value::Bool(true))) {
                            shop::handle_shop_key(c, key);
                            return;
                        }
                        let n = GAME_MODES.len();
                        let changed = {
                            let mut idx = sel.lock().unwrap();
                            match key {
                                Key::Named(NamedKey::ArrowLeft) => {
                                    *idx = (*idx + n - 1) % n;
                                    true
                                }
                                Key::Named(NamedKey::ArrowRight) => {
                                    *idx = (*idx + 1) % n;
                                    true
                                }
                                _ => false,
                            }
                        };
                        if changed {
                            let idx = *sel.lock().unwrap();
                            let (mode_name, mode_desc) = GAME_MODES[idx];
                            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                                let s = c.virtual_scale();
                                if let Some(obj) = c.get_game_object_mut("menu_mode_name_text") {
                                    obj.set_drawable(Box::new(ui_text_spec(mode_name, &font, 36.0 * s, Color(200, 240, 255, 255), 640.0 * s)));
                                }
                                if let Some(obj) = c.get_game_object_mut("menu_mode_desc_text") {
                                    obj.set_drawable(Box::new(ui_text_spec(mode_desc, &font, 18.0 * s, Color(140, 190, 240, 200), 800.0 * s)));
                                }
                            }
                        }
                    }
                });
                canvas.on_key_release(|c, key| {
                    if !c.is_scene("menu") { return; }
                    if matches!(c.get_var("menu_in_shop"), Some(Value::Bool(true))) {
                        shop::handle_shop_key_release(c, key);
                    }
                });
                canvas.set_var("menu_key_registered", true);
            }

            let menu_anim_registered = matches!(canvas.get_var("menu_anim_registered"), Some(Value::Bool(true)));
            if !menu_anim_registered {
                canvas.on_update(|c| {
                    if !c.is_scene("menu") { return; }

                    // ── Camera pan (shop ↔ menu) ────────────────────────
                    let target_y = c.get_f32("menu_cam_target_y");
                    if let Some(cam) = c.camera_mut() {
                        let diff = target_y - cam.position.1;
                        if diff.abs() < 2.0 {
                            cam.position.1 = target_y;
                        } else {
                            cam.position.1 += diff * 0.12;
                        }
                    }

                    // ── Shop carousel tick ──────────────────────────────
                    if matches!(c.get_var("menu_in_shop"), Some(Value::Bool(true))) {
                        shop::tick_shop(c);
                    }

                    // ── Menu BGM alternation ───────────────────────────
                    if audio_state::menu_bgm_finished() {
                        let cur = c.get_i32("menu_bgm_track_index").max(0) as usize;
                        let next = (cur + 1) % MENU_TRACKS.len();
                        play_menu_track(c, next);
                    }

                    if matches!(c.get_var("menu_text_dirty"), Some(Value::Bool(true))) {
                        if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                            let s = c.virtual_scale();
                            if let Some(obj) = c.get_game_object_mut("menu_title_text") {
                                obj.set_drawable(Box::new(ui_text_spec("ball_swing", &font, 58.0 * s, Color(0, 0, 0, 255), 1700.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_sub_text") {
                                obj.set_drawable(Box::new(ui_text_spec("SELECT   MODE", &font, 18.0 * s, Color(180, 220, 255, 220), 600.0 * s)));
                            }
                            let (mode_name, mode_desc) = GAME_MODES[0];
                            if let Some(obj) = c.get_game_object_mut("menu_mode_name_text") {
                                obj.set_drawable(Box::new(ui_text_spec(mode_name, &font, 36.0 * s, Color(200, 240, 255, 255), 640.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_mode_desc_text") {
                                obj.set_drawable(Box::new(ui_text_spec(mode_desc, &font, 18.0 * s, Color(140, 190, 240, 200), 800.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_start_text") {
                                obj.set_drawable(Box::new(ui_text_spec("SPACE   \u{2022}   CLICK   TO   PLAY", &font, 20.0 * s, Color(0, 0, 0, 255), 540.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_shop_text") {
                                obj.set_drawable(Box::new(ui_text_spec("SHOP", &font, 36.0 * s, Color(255, 240, 200, 255), 420.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_settings_text") {
                                obj.set_drawable(Box::new(ui_text_spec("SETTINGS", &font, 36.0 * s, Color(200, 240, 255, 255), 420.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_achievements_text") {
                                obj.set_drawable(Box::new(ui_text_spec("ACHIEVEMENTS", &font, 22.0 * s, Color(220, 180, 255, 255), 280.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_stats_text") {
                                obj.set_drawable(Box::new(ui_text_spec("STATS", &font, 26.0 * s, Color(255, 210, 160, 255), 280.0 * s)));
                            }
                            if let Some(obj) = c.get_game_object_mut("menu_daily_text") {
                                obj.set_drawable(Box::new(ui_text_spec("DAILY REWARD", &font, 22.0 * s, Color(180, 255, 200, 255), 280.0 * s)));
                            }
                        }
                        c.set_var("menu_text_dirty", false);
                    }

                    if !matches!(c.get_var("menu_ui_animating"), Some(Value::Bool(true))) { return; }

                    let mut remaining = c.get_i32("menu_ui_anim_frames").max(0);
                    let total = c.get_i32("menu_ui_anim_total").max(1);
                    if remaining <= 0 {
                        c.set_var("menu_ui_animating", false);
                        return;
                    }

                    remaining -= 1;
                    let t = 1.0 - (remaining as f32 / total as f32);
                    let ease = 1.0 - (1.0 - t).powi(3);
                    let off = (1.0 - ease) * -VW;

                    if let Some(obj) = c.get_game_object_mut("menu_title") {
                        obj.position.0 = off + (VW/2.0 - 1700.0/2.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_sub") {
                        obj.position.0 = off + (VW/2.0 - 600.0/2.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_mode_selector") {
                        obj.position.0 = off + (VW/2.0 - 400.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("start_btn") {
                        obj.position.0 = off + (VW/2.0 - 540.0/2.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_title_text") {
                        obj.position.0 = off + (VW * 0.5 - 850.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_sub_text") {
                        obj.position.0 = off + (VW * 0.5 - 300.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_mode_name_text") {
                        obj.position.0 = off + (VW * 0.5 - 320.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_mode_desc_text") {
                        obj.position.0 = off + (VW * 0.5 - 400.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_start_text") {
                        obj.position.0 = off + (VW * 0.5 - 270.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_shop_btn") {
                        obj.position.0 = off + (VW / 2.0 - 420.0 - 20.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_settings_btn") {
                        obj.position.0 = off + (VW / 2.0 + 20.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_shop_text") {
                        obj.position.0 = off + (VW / 2.0 - 420.0 - 20.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_settings_text") {
                        obj.position.0 = off + (VW / 2.0 + 20.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_achievements_btn") {
                        obj.position.0 = off + (VW / 2.0 - 440.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_stats_btn") {
                        obj.position.0 = off + (VW / 2.0 - 140.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_daily_btn") {
                        obj.position.0 = off + (VW / 2.0 + 160.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_achievements_text") {
                        obj.position.0 = off + (VW / 2.0 - 440.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_stats_text") {
                        obj.position.0 = off + (VW / 2.0 - 140.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("menu_daily_text") {
                        obj.position.0 = off + (VW / 2.0 + 160.0);
                    }

                    c.set_var("menu_ui_anim_frames", remaining);
                    if remaining == 0 {
                        c.set_var("menu_ui_animating", false);
                    }
                });
                canvas.set_var("menu_anim_registered", true);
            }

            canvas.register_custom_event("goto_game".into(), |c| c.load_scene("game"));
            // Pan camera upward into the shop region (no scene switch).
            canvas.register_custom_event("goto_shop".into(), |c| {
                c.set_var("menu_cam_target_y", 0.0f32);
                c.set_var("menu_in_shop", true);
                shop::init_shop(c);
            });
            // Back: if on carousel screen return to categories; otherwise return to menu.
            canvas.register_custom_event("shop_back".into(), |c| {
                if c.get_i32("shop_screen") == 1 {
                    shop::show_categories(c);
                } else {
                    c.set_var("menu_cam_target_y", MENU_Y);
                    c.set_var("menu_in_shop", false);
                }
            });
            // Confirm selection for the active category and return to categories.
            canvas.register_custom_event("shop_select".into(), |c| {
                let sel = c.get_i32("shop_selected");
                let cat = c.get_i32("shop_active_category");
                match cat {
                    0 => c.set_var("player_char_selected", sel),
                    1 => c.set_var("player_rope_selected", sel),
                    2 => c.set_var("player_bg_selected",   sel),
                    3 => c.set_var("player_trail_selected", sel),
                    _ => {}
                }
                shop::show_categories(c);
            });
            // Category button events — each opens the carousel for that category.
            canvas.register_custom_event("shop_cat_0".into(), |c| { shop::show_carousel(c, 0); });
            canvas.register_custom_event("shop_cat_1".into(), |c| { shop::show_carousel(c, 1); });
            canvas.register_custom_event("shop_cat_2".into(), |c| { shop::show_carousel(c, 2); });
            canvas.register_custom_event("shop_cat_3".into(), |c| { shop::show_carousel(c, 3); });
            canvas.register_custom_event("shop_cat_4".into(), |c| { shop::show_carousel(c, 4); });
            canvas.register_custom_event("goto_menu_settings".into(), |c| c.load_scene("menu_settings"));
            canvas.register_custom_event("goto_achievements".into(), |c| c.load_scene("achievements"));
            canvas.register_custom_event("goto_stats".into(), |c| c.load_scene("stats"));
            canvas.register_custom_event("goto_daily_reward".into(), |c| c.load_scene("daily_reward"));
        })
}

// ── Stand-alone Settings scene (accessible from main menu) ──────────────────
// Mirrors the in-game settings panel: same toggle vars, same keys (Q/W/E/R/T/Y/U/I).

pub fn build_menu_settings_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "ms_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "ms_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(40, 80, 160, 140))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // Panel background
    let panel = {
        let (pw, ph) = (1700u32, 750u32);
        let mut img = image::RgbaImage::new(pw, ph);
        for py in 0..ph { for px in 0..pw {
            let border = px < 4 || px >= pw - 4 || py < 4 || py >= ph - 4;
            img.put_pixel(px, py, image::Rgba([
                if border { 90 } else { 14 },
                if border { 150 } else { 22 },
                if border { 220 } else { 45 },
                if border { 255 } else { 230 },
            ]));
        }}
        GameObject::new_rect(
            ctx, "ms_panel".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (pw as f32, ph as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (pw as f32, ph as f32),
            (VW / 2.0 - pw as f32 / 2.0, VH * 0.24),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    // Title text
    let title_obj = GameObject::build("ms_title_text")
        .size(1400.0, 200.0)
        .position(VW * 0.5 - 700.0, VH * 0.08)
        .tag("ui")
        .build(ctx);

    // Settings text (toggle display)
    let settings_text_obj = GameObject::build("ms_settings_text")
        .size(1600.0, 600.0)
        .position(VW / 2.0 - 800.0, VH * 0.24 + 60.0)
        .tag("ui")
        .build(ctx);

    // Back button
    let back_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            img.put_pixel(px, py, image::Rgba([50, 80, 130, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, "ms_back_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.86),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let back_text_obj = GameObject::build("ms_back_text")
        .size(420.0, 110.0)
        .position(VW / 2.0 - 210.0, VH * 0.86 + (110.0 - 36.0) / 2.0)
        .tag("ui")
        .build(ctx);

    Scene::new("menu_settings")
        .with_object("ms_bg",            bg)
        .with_object("ms_bg_tint",       bg_tint)
        .with_object("ms_panel",         panel)
        .with_object("ms_title_text",    title_obj)
        .with_object("ms_settings_text", settings_text_obj)
        .with_object("ms_back_btn",      back_btn)
        .with_object("ms_back_text",     back_text_obj)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "ms_back".into() },
                target: Target::name("ms_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("ms_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if canvas.get_var("vol_master").is_none() {
                canvas.set_var("vol_master", 1.0f32);
            }
            if canvas.get_var("vol_music").is_none() {
                canvas.set_var("vol_music", 1.0f32);
            }
            if canvas.get_var("vol_sound").is_none() {
                canvas.set_var("vol_sound", 1.0f32);
            }

            // Render text
            menu_settings_update_text(canvas);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("ms_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "SETTINGS", &font, 72.0 * s, Color(200, 230, 255, 255), 1400.0 * s,
                    )));
                }
                if let Some(obj) = canvas.get_game_object_mut("ms_back_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "\u{25C4}  BACK", &font, 32.0 * s, Color(220, 235, 255, 255), 420.0 * s,
                    )));
                }
            }

            // Key handler for toggles
            let ms_key_registered = matches!(canvas.get_var("ms_key_registered"), Some(Value::Bool(true)));
            if !ms_key_registered {
                canvas.on_key_press(|c, key| {
                    if !c.is_scene("menu_settings") { return; }
                    let adjust = match key {
                        Key::Character(ch) if ch == "a" => Some(("vol_master", -0.05f32)),
                        Key::Character(ch) if ch == "d" => Some(("vol_master",  0.05f32)),
                        Key::Character(ch) if ch == "j" => Some(("vol_music",  -0.05f32)),
                        Key::Character(ch) if ch == "l" => Some(("vol_music",   0.05f32)),
                        Key::Character(ch) if ch == "n" => Some(("vol_sound",  -0.05f32)),
                        Key::Character(ch) if ch == "m" => Some(("vol_sound",   0.05f32)),
                        _ => None,
                    };
                    if let Some((var, delta)) = adjust {
                        let cur = volume_value(c, var, 1.0);
                        set_volume_value(c, var, cur + delta);
                        menu_settings_update_text(c);
                    }
                });
                canvas.set_var("ms_key_registered", true);
            }

            canvas.register_custom_event("ms_back".into(), |c| c.load_scene("menu"));
        })
}

fn menu_settings_update_text(c: &mut Canvas) {
    let master = volume_value(c, "vol_master", 1.0);
    let music = volume_value(c, "vol_music", 1.0);
    let sound = volume_value(c, "vol_sound", 1.0);
    let text = format!(
        "MASTER VOLUME\n  [{}] {:>3}%\n\
         MUSIC VOLUME\n  [{}] {:>3}%\n\
         SOUND VOLUME\n  [{}] {:>3}%\n\
         CONTROLS: [A]/[D] MASTER   [J]/[L] MUSIC   [N]/[M] SOUND",
        slider_bar(master, 20),
        (master * 100.0).round() as i32,
        slider_bar(music, 20),
        (music * 100.0).round() as i32,
        slider_bar(sound, 20),
        (sound * 100.0).round() as i32,
    );
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("ms_settings_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &text, &font, 38.0 * s, Color(235, 245, 255, 255), 1600.0 * s,
            )));
        }
    }
}

pub fn build_gameover_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "go_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "go_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(230, 50, 50, 120))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let title = {
        let (w, h) = (1300u32, 230u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let t = py as f32 / h as f32;
            img.put_pixel(px, py, image::Rgba([255, (90.0*(1.0-t)) as u8, 40, 255]));
        }}
        GameObject::new_rect(
            ctx, "go_title".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.20),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let dist_bar = GameObject::new_rect(
        ctx, "go_dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
            image: bar_img(600, 44, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (600.0, 44.0), (VW/2.0 - 300.0, VH*0.37),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let make_btn = |ctx: &mut Context, id: &str, (r,g,b): (u8,u8,u8), y: f32| {
        let (w, h) = (520u32, 130u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([r, g, b, if border {255} else {200}]));
        }}
        GameObject::new_rect(
            ctx, id.to_string().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, y),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let retry_btn   = make_btn(ctx, "retry_btn",   (50, 160, 90),  VH*0.66);
    let go_menu_btn = make_btn(ctx, "go_menu_btn", (50,  80, 160), VH*0.80);

    let go_title_text = GameObject::build("go_title_text")
        .size(1300.0, 230.0)
        .position(VW * 0.5 - 650.0, VH * 0.20 + (230.0 - 74.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let go_retry_text = GameObject::build("go_retry_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.66 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let go_menu_text = GameObject::build("go_menu_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.80 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let go_stats_text = GameObject::build("go_stats_text")
        .size(1000.0, 180.0)
        .position(VW * 0.5 - 500.0, VH * 0.44)
        .tag("ui")
        .build(ctx);

    let go_stats_box = {
        let (w, h) = (1060u32, 200u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            if border {
                img.put_pixel(px, py, image::Rgba([180, 200, 220, 200]));
            } else {
                img.put_pixel(px, py, image::Rgba([20, 15, 30, 200]));
            }
        }}
        GameObject::new_rect(
            ctx, "go_stats_box".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.44 - 10.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    Scene::new("gameover")
        .with_object("go_bg",       bg)
        .with_object("go_bg_tint",  bg_tint)
        .with_object("go_title",    title)
        .with_object("go_dist_bar", dist_bar)
        .with_object("go_stats_box", go_stats_box)
        .with_object("retry_btn",   retry_btn)
        .with_object("go_menu_btn", go_menu_btn)
        .with_object("go_title_text", go_title_text)
        .with_object("go_retry_text", go_retry_text)
        .with_object("go_menu_text", go_menu_text)
        .with_object("go_stats_text", go_stats_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "go_retry".into() },
                target: Target::name("retry_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("retry_btn"),
        )
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "go_retry".into() },
                target: Target::name("retry_btn"),
            },
            Target::name("retry_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "go_menu".into() },
                target: Target::name("go_menu_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("go_menu_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                let last_distance = canvas.get_f32("last_distance");
                let last_coins = canvas.get_i32("last_coins").max(0);
                let died_to_oxygen = matches!(canvas.get_var("died_to_oxygen"), Some(Value::Bool(true)));
                let dist_fill = (last_distance / 40000.0).clamp(0.0, 1.0);

                if let Some(obj) = canvas.get_game_object_mut("go_dist_bar") {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
                        image: bar_img(600, 44, dist_fill, 80, 220, 160).into(),
                        color: None,
                    });
                }

                if let Some(obj) = canvas.get_game_object_mut("go_title_text") {
                    let title = if died_to_oxygen { "YOU RAN OUT OF OXYGEN" } else { "YOU FELL" };
                    let title_size = if died_to_oxygen { 50.0 * s } else { 58.0 * s };
                    obj.set_drawable(Box::new(ui_text_spec(title, &font, title_size, Color(0, 0, 0, 255), 1300.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("go_retry_text") {
                    obj.set_drawable(Box::new(ui_text_spec("RETRY", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("go_menu_text") {
                    obj.set_drawable(Box::new(ui_text_spec("MENU", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("go_stats_text") {
                    let stats_line = format!("DISTANCE  {:05}\nCOINS  {:03}", last_distance as i32, last_coins);
                    obj.set_drawable(Box::new(ui_text_spec(&stats_line, &font, 50.0 * s, Color(255, 255, 255, 255), 1000.0 * s)));
                }

                if died_to_oxygen {
                    canvas.set_var("died_to_oxygen", false);
                }
            }

            canvas.register_custom_event("go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("go_menu".into(),  |c| c.load_scene("menu"));
        })
}

// ── Sun-death game-over screen ────────────────────────────────────────────────
// Identical layout to build_gameover_scene but with a solar theme and the text
// "YOU FLEW TOO CLOSE TO THE SUN".

pub fn build_gameover_sun_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "sun_go_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "sun_go_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(230, 120, 20, 150))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // Wider title banner to fit the long text.
    let title = {
        let (w, h) = (1900u32, 230u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let t = py as f32 / h as f32;
            img.put_pixel(px, py, image::Rgba([255, (160.0 - 80.0 * t) as u8, 10, 255]));
        }}
        GameObject::new_rect(
            ctx, "sun_go_title".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.20),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let dist_bar = GameObject::new_rect(
        ctx, "sun_go_dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
            image: bar_img(600, 44, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (600.0, 44.0), (VW / 2.0 - 300.0, VH * 0.37),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let make_btn = |ctx: &mut Context, id: &str, (r, g, b): (u8, u8, u8), y: f32| {
        let (w, h) = (520u32, 130u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([r, g, b, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, id.to_string().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, y),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let retry_btn   = make_btn(ctx, "sun_retry_btn",   (160, 80, 20), VH * 0.66);
    let go_menu_btn = make_btn(ctx, "sun_go_menu_btn", (50, 80, 160), VH * 0.80);

    let sun_go_title_text = GameObject::build("sun_go_title_text")
        .size(1900.0, 230.0)
        .position(VW * 0.5 - 950.0, VH * 0.20 + (230.0 - 74.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let sun_go_retry_text = GameObject::build("sun_go_retry_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.66 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let sun_go_menu_text = GameObject::build("sun_go_menu_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.80 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let sun_go_stats_text = GameObject::build("sun_go_stats_text")
        .size(1000.0, 180.0)
        .position(VW * 0.5 - 500.0, VH * 0.44)
        .tag("ui")
        .build(ctx);

    let sun_go_stats_box = {
        let (w, h) = (1060u32, 200u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            if border {
                img.put_pixel(px, py, image::Rgba([255, 180, 80, 200]));
            } else {
                img.put_pixel(px, py, image::Rgba([30, 15, 5, 200]));
            }
        }}
        GameObject::new_rect(
            ctx, "sun_go_stats_box".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.44 - 10.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    Scene::new("gameover_sun")
        .with_object("sun_go_bg",        bg)
        .with_object("sun_go_bg_tint",   bg_tint)
        .with_object("sun_go_title",     title)
        .with_object("sun_go_dist_bar",  dist_bar)
        .with_object("sun_go_stats_box", sun_go_stats_box)
        .with_object("sun_retry_btn",    retry_btn)
        .with_object("sun_go_menu_btn",  go_menu_btn)
        .with_object("sun_go_title_text", sun_go_title_text)
        .with_object("sun_go_retry_text", sun_go_retry_text)
        .with_object("sun_go_menu_text",  sun_go_menu_text)
        .with_object("sun_go_stats_text", sun_go_stats_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "sun_go_retry".into() },
                target: Target::name("sun_retry_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("sun_retry_btn"),
        )
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "sun_go_retry".into() },
                target: Target::name("sun_retry_btn"),
            },
            Target::name("sun_retry_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "sun_go_menu".into() },
                target: Target::name("sun_go_menu_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("sun_go_menu_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                let last_distance = canvas.get_f32("last_distance");
                let last_coins = canvas.get_i32("last_coins").max(0);
                let dist_fill = (last_distance / 40000.0).clamp(0.0, 1.0);

                if let Some(obj) = canvas.get_game_object_mut("sun_go_dist_bar") {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
                        image: bar_img(600, 44, dist_fill, 80, 220, 160).into(),
                        color: None,
                    });
                }

                if let Some(obj) = canvas.get_game_object_mut("sun_go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "YOU FLEW TOO CLOSE TO THE SUN",
                        &font, 48.0 * s,
                        Color(0, 0, 0, 255),
                        1900.0 * s,
                    )));
                }

                if let Some(obj) = canvas.get_game_object_mut("sun_go_retry_text") {
                    obj.set_drawable(Box::new(ui_text_spec("RETRY", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("sun_go_menu_text") {
                    obj.set_drawable(Box::new(ui_text_spec("MENU", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("sun_go_stats_text") {
                    let stats_line = format!("DISTANCE  {:05}\nCOINS  {:03}", last_distance as i32, last_coins);
                    obj.set_drawable(Box::new(ui_text_spec(&stats_line, &font, 50.0 * s, Color(255, 255, 255, 255), 1000.0 * s)));
                }
            }

            canvas.register_custom_event("sun_go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("sun_go_menu".into(),  |c| c.load_scene("menu"));
        })
}

pub fn build_gameover_oxygen_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "oxy_go_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "oxy_go_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(30, 170, 210, 150))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let title = {
        let (w, h) = (1700u32, 230u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let t = py as f32 / h as f32;
            img.put_pixel(px, py, image::Rgba([(130.0 - 40.0 * t) as u8, 240, 255, 255]));
        }}
        GameObject::new_rect(
            ctx, "oxy_go_title".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.20),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let dist_bar = GameObject::new_rect(
        ctx, "oxy_go_dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
            image: bar_img(600, 44, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (600.0, 44.0), (VW / 2.0 - 300.0, VH * 0.37),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let make_btn = |ctx: &mut Context, id: &str, (r, g, b): (u8, u8, u8), y: f32| {
        let (w, h) = (520u32, 130u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([r, g, b, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, id.to_string().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, y),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let retry_btn   = make_btn(ctx, "oxy_retry_btn",   (50, 160, 90), VH * 0.66);
    let go_menu_btn = make_btn(ctx, "oxy_go_menu_btn", (50, 80, 160), VH * 0.80);

    let oxy_go_title_text = GameObject::build("oxy_go_title_text")
        .size(1700.0, 230.0)
        .position(VW * 0.5 - 850.0, VH * 0.20 + (230.0 - 74.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let oxy_go_retry_text = GameObject::build("oxy_go_retry_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.66 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let oxy_go_menu_text = GameObject::build("oxy_go_menu_text")
        .size(520.0, 130.0)
        .position(VW * 0.5 - 260.0, VH * 0.80 + (130.0 - 54.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let oxy_go_stats_text = GameObject::build("oxy_go_stats_text")
        .size(1000.0, 180.0)
        .position(VW * 0.5 - 500.0, VH * 0.44)
        .tag("ui")
        .build(ctx);

    let oxy_go_stats_box = {
        let (w, h) = (1060u32, 200u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            if border {
                img.put_pixel(px, py, image::Rgba([120, 220, 255, 200]));
            } else {
                img.put_pixel(px, py, image::Rgba([8, 25, 35, 200]));
            }
        }}
        GameObject::new_rect(
            ctx, "oxy_go_stats_box".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.44 - 10.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    Scene::new("gameover_oxygen")
        .with_object("oxy_go_bg",        bg)
        .with_object("oxy_go_bg_tint",   bg_tint)
        .with_object("oxy_go_title",     title)
        .with_object("oxy_go_dist_bar",  dist_bar)
        .with_object("oxy_go_stats_box", oxy_go_stats_box)
        .with_object("oxy_retry_btn",    retry_btn)
        .with_object("oxy_go_menu_btn",  go_menu_btn)
        .with_object("oxy_go_title_text", oxy_go_title_text)
        .with_object("oxy_go_retry_text", oxy_go_retry_text)
        .with_object("oxy_go_menu_text",  oxy_go_menu_text)
        .with_object("oxy_go_stats_text", oxy_go_stats_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "oxy_go_retry".into() },
                target: Target::name("oxy_retry_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("oxy_retry_btn"),
        )
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "oxy_go_retry".into() },
                target: Target::name("oxy_retry_btn"),
            },
            Target::name("oxy_retry_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "oxy_go_menu".into() },
                target: Target::name("oxy_go_menu_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("oxy_go_menu_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                let last_distance = canvas.get_f32("last_distance");
                let last_coins = canvas.get_i32("last_coins").max(0);
                let dist_fill = (last_distance / 40000.0).clamp(0.0, 1.0);

                if let Some(obj) = canvas.get_game_object_mut("oxy_go_dist_bar") {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
                        image: bar_img(600, 44, dist_fill, 80, 220, 160).into(),
                        color: None,
                    });
                }

                if let Some(obj) = canvas.get_game_object_mut("oxy_go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "YOU RAN OUT OF OXYGEN",
                        &font, 52.0 * s,
                        Color(0, 0, 0, 255),
                        1700.0 * s,
                    )));
                }

                if let Some(obj) = canvas.get_game_object_mut("oxy_go_retry_text") {
                    obj.set_drawable(Box::new(ui_text_spec("RETRY", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("oxy_go_menu_text") {
                    obj.set_drawable(Box::new(ui_text_spec("MENU", &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
                }

                if let Some(obj) = canvas.get_game_object_mut("oxy_go_stats_text") {
                    let stats_line = format!("DISTANCE  {:05}\nCOINS  {:03}", last_distance as i32, last_coins);
                    obj.set_drawable(Box::new(ui_text_spec(&stats_line, &font, 50.0 * s, Color(255, 255, 255, 255), 1000.0 * s)));
                }
            }

            canvas.register_custom_event("oxy_go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("oxy_go_menu".into(),  |c| c.load_scene("menu"));
        })
}

// ── Achievements scene ───────────────────────────────────────────────────────

pub fn build_achievements_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "ach_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "ach_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(80, 40, 140, 140))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let back_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            img.put_pixel(px, py, image::Rgba([50, 80, 130, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, "ach_back_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.86),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let title_obj  = GameObject::build("ach_title_text").size(1400.0, 200.0).position(VW * 0.5 - 700.0, VH * 0.08).tag("ui").build(ctx);
    let body_obj   = GameObject::build("ach_body_text").size(1600.0, 600.0).position(VW / 2.0 - 800.0, VH * 0.28).tag("ui").build(ctx);
    let back_text  = GameObject::build("ach_back_text").size(420.0, 110.0).position(VW / 2.0 - 210.0, VH * 0.86 + (110.0 - 36.0) / 2.0).tag("ui").build(ctx);

    Scene::new("achievements")
        .with_object("ach_bg",         bg)
        .with_object("ach_bg_tint",    bg_tint)
        .with_object("ach_back_btn",   back_btn)
        .with_object("ach_title_text", title_obj)
        .with_object("ach_body_text",  body_obj)
        .with_object("ach_back_text",  back_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "ach_back".into() },
                target: Target::name("ach_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("ach_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("ach_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("ACHIEVEMENTS", &font, 72.0 * s, Color(220, 180, 255, 255), 1400.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("ach_body_text") {
                    obj.set_drawable(Box::new(ui_text_spec("Coming soon!", &font, 48.0 * s, Color(200, 200, 220, 200), 1600.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("ach_back_text") {
                    obj.set_drawable(Box::new(ui_text_spec("\u{25C4}  BACK", &font, 32.0 * s, Color(220, 235, 255, 255), 420.0 * s)));
                }
            }

            canvas.register_custom_event("ach_back".into(), |c| c.load_scene("menu"));
        })
}

// ── Stats scene ──────────────────────────────────────────────────────────────

pub fn build_stats_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "stats_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "stats_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(140, 80, 40, 140))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let back_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            img.put_pixel(px, py, image::Rgba([50, 80, 130, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, "stats_back_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.86),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let title_obj = GameObject::build("stats_title_text").size(1400.0, 200.0).position(VW * 0.5 - 700.0, VH * 0.08).tag("ui").build(ctx);
    let body_obj  = GameObject::build("stats_body_text").size(1600.0, 600.0).position(VW / 2.0 - 800.0, VH * 0.28).tag("ui").build(ctx);
    let back_text = GameObject::build("stats_back_text").size(420.0, 110.0).position(VW / 2.0 - 210.0, VH * 0.86 + (110.0 - 36.0) / 2.0).tag("ui").build(ctx);

    Scene::new("stats")
        .with_object("stats_bg",         bg)
        .with_object("stats_bg_tint",    bg_tint)
        .with_object("stats_back_btn",   back_btn)
        .with_object("stats_title_text", title_obj)
        .with_object("stats_body_text",  body_obj)
        .with_object("stats_back_text",  back_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "stats_back".into() },
                target: Target::name("stats_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("stats_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("stats_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("STATS", &font, 72.0 * s, Color(255, 210, 160, 255), 1400.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("stats_body_text") {
                    obj.set_drawable(Box::new(ui_text_spec("Coming soon!", &font, 48.0 * s, Color(220, 200, 180, 200), 1600.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("stats_back_text") {
                    obj.set_drawable(Box::new(ui_text_spec("\u{25C4}  BACK", &font, 32.0 * s, Color(220, 235, 255, 255), 420.0 * s)));
                }
            }

            canvas.register_custom_event("stats_back".into(), |c| c.load_scene("menu"));
        })
}

// ── Daily Reward scene ───────────────────────────────────────────────────────

pub fn build_daily_reward_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "daily_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "daily_bg_tint".into(),
        Some(tint_overlay(VW + 800.0, VH, Color(40, 140, 80, 140))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let back_btn = {
        let (w, h) = (420u32, 110u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            img.put_pixel(px, py, image::Rgba([50, 80, 130, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, "daily_back_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.86),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let title_obj = GameObject::build("daily_title_text").size(1400.0, 200.0).position(VW * 0.5 - 700.0, VH * 0.08).tag("ui").build(ctx);
    let body_obj  = GameObject::build("daily_body_text").size(1600.0, 600.0).position(VW / 2.0 - 800.0, VH * 0.28).tag("ui").build(ctx);
    let back_text = GameObject::build("daily_back_text").size(420.0, 110.0).position(VW / 2.0 - 210.0, VH * 0.86 + (110.0 - 36.0) / 2.0).tag("ui").build(ctx);

    Scene::new("daily_reward")
        .with_object("daily_bg",         bg)
        .with_object("daily_bg_tint",    bg_tint)
        .with_object("daily_back_btn",   back_btn)
        .with_object("daily_title_text", title_obj)
        .with_object("daily_body_text",  body_obj)
        .with_object("daily_back_text",  back_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "daily_back".into() },
                target: Target::name("daily_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("daily_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("daily_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("DAILY REWARD", &font, 72.0 * s, Color(180, 255, 200, 255), 1400.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("daily_body_text") {
                    obj.set_drawable(Box::new(ui_text_spec("Coming soon!", &font, 48.0 * s, Color(180, 220, 195, 200), 1600.0 * s)));
                }
                if let Some(obj) = canvas.get_game_object_mut("daily_back_text") {
                    obj.set_drawable(Box::new(ui_text_spec("\u{25C4}  BACK", &font, 32.0 * s, Color(220, 235, 255, 255), 420.0 * s)));
                }
            }

            canvas.register_custom_event("daily_back".into(), |c| c.load_scene("menu"));
        })
}
