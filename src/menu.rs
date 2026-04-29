use quartz::*;
use std::sync::{Arc, Mutex};
use crate::constants::*;
use crate::images::*;
use crate::objects::ui_text_spec;

const MENU_UI_ANIM_FRAMES: i32 = 60;

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
        Some(load_image_sized(ASSET_BACKGROUND_2, VW, VH)),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "menu_bg_tint".into(),
        Some(tint_overlay(VW, VH, Color(70, 120, 255, 110))),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
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
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.14),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

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
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.40),
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
        (800.0, 140.0), (VW/2.0 - 400.0, VH*0.46),
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
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.68),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let menu_title_text = GameObject::build("menu_title_text")
        .size(1700.0, 260.0)
        .position(VW * 0.5 - 850.0, VH * 0.14 + (260.0 - 74.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_sub_text = GameObject::build("menu_sub_text")
        .size(600.0, 60.0)
        .position(VW * 0.5 - 300.0, VH * 0.40 + (60.0 - 22.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_mode_name_text = GameObject::build("menu_mode_name_text")
        .size(640.0, 140.0)
        .position(VW * 0.5 - 320.0, VH * 0.46 + (140.0 - 52.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let menu_mode_desc_text = GameObject::build("menu_mode_desc_text")
        .size(800.0, 60.0)
        .position(VW * 0.5 - 400.0, VH * 0.46 + 152.0)
        .tag("ui")
        .build(ctx);

    let menu_start_text = GameObject::build("menu_start_text")
        .size(540.0, 130.0)
        .position(VW * 0.5 - 270.0, VH * 0.68 + (130.0 - 24.0) / 2.0)
        .tag("ui")
        .build(ctx);

    let scene = Scene::new("menu")
        .with_object("menu_bg",             bg)
        .with_object("menu_bg_tint",        bg_tint)
        .with_object("menu_title",          title)
        .with_object("menu_sub",            menu_sub)
        .with_object("menu_mode_selector",  menu_mode_selector)
        .with_object("start_btn",           start_btn)
        .with_object("menu_title_text",     menu_title_text)
        .with_object("menu_sub_text",       menu_sub_text)
        .with_object("menu_mode_name_text", menu_mode_name_text)
        .with_object("menu_mode_desc_text", menu_mode_desc_text)
        .with_object("menu_start_text",     menu_start_text);

    scene
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "goto_game".into() },
                target: Target::name("start_btn"),
                modifiers: None,
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
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            // Slide menu UI in from the left on every menu entry.
            let off = -VW;
            if let Some(obj) = canvas.get_game_object_mut("menu_title") {
                obj.position.0 = off + (VW/2.0 - 1700.0/2.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_sub") {
                obj.position.0 = off + (VW/2.0 - 600.0/2.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_mode_selector") {
                obj.position.0 = off + (VW/2.0 - 400.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("start_btn") {
                obj.position.0 = off + (VW/2.0 - 540.0/2.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_title_text") {
                obj.position.0 = off + (VW * 0.5 - 850.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_sub_text") {
                obj.position.0 = off + (VW * 0.5 - 300.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_mode_name_text") {
                obj.position.0 = off + (VW * 0.5 - 320.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_mode_desc_text") {
                obj.position.0 = off + (VW * 0.5 - 400.0);
            }
            if let Some(obj) = canvas.get_game_object_mut("menu_start_text") {
                obj.position.0 = off + (VW * 0.5 - 270.0);
            }
            canvas.set_var("menu_ui_animating", true);
            canvas.set_var("menu_ui_anim_frames", MENU_UI_ANIM_FRAMES);
            canvas.set_var("menu_ui_anim_total", MENU_UI_ANIM_FRAMES);
            canvas.set_var("menu_text_dirty", true);

            let selected = Arc::new(Mutex::new(0usize));

            let menu_key_registered = matches!(canvas.get_var("menu_key_registered"), Some(Value::Bool(true)));
            if !menu_key_registered {
                canvas.on_key_press({
                    let sel = Arc::clone(&selected);
                    move |c, key| {
                        if !c.is_scene("menu") { return; }

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
                canvas.set_var("menu_key_registered", true);
            }

            let menu_anim_registered = matches!(canvas.get_var("menu_anim_registered"), Some(Value::Bool(true)));
            if !menu_anim_registered {
                canvas.on_update(|c| {
                    if !c.is_scene("menu") { return; }

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

                    c.set_var("menu_ui_anim_frames", remaining);
                    if remaining == 0 {
                        c.set_var("menu_ui_animating", false);
                    }
                });
                canvas.set_var("menu_anim_registered", true);
            }

            canvas.register_custom_event("goto_game".into(), |c| c.load_scene("game"));
        })
}

pub fn build_gameover_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx, "go_bg".into(),
        Some(load_image_sized(ASSET_BACKGROUND_2, VW, VH)),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "go_bg_tint".into(),
        Some(tint_overlay(VW, VH, Color(230, 50, 50, 120))),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
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
                modifiers: None,
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
                let dist_fill = (last_distance / 40000.0).clamp(0.0, 1.0);

                if let Some(obj) = canvas.get_game_object_mut("go_dist_bar") {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0),
                        image: bar_img(600, 44, dist_fill, 80, 220, 160).into(),
                        color: None,
                    });
                }

                if let Some(obj) = canvas.get_game_object_mut("go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("YOU FELL", &font, 58.0 * s, Color(0, 0, 0, 255), 1300.0 * s)));
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
        Some(load_image_sized(ASSET_BACKGROUND_2, VW, VH)),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    let bg_tint = GameObject::new_rect(
        ctx, "sun_go_bg_tint".into(),
        Some(tint_overlay(VW, VH, Color(230, 120, 20, 150))),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
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
                modifiers: None,
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
