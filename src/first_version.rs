use quartz::*;
use ramp::prism;

// ── Virtual resolution ─────────────────────────────────────────────────────────
const VW: f32 = 3840.0;
const VH: f32 = 2160.0;

// ── World constants ────────────────────────────────────────────────────────────
const WORLD_SIZE: f32 = 80_000.0; // enormous scrollable world

// ── Ship constants ─────────────────────────────────────────────────────────────
const SHIP_W: f32 = 80.0;
const SHIP_H: f32 = 80.0;
const THRUST_FORCE: f32 = 0.45;
const ROTATION_SPEED: f32 = 3.5; // degrees per tick
const MAX_SPEED: f32 = 18.0;
const SAFE_LAND_SPEED: f32 = 4.5; // max speed to land without hull damage

// ── Resource drain rates (per tick) ───────────────────────────────────────────
const FUEL_DRAIN: f32 = 0.012; // drains when thrusting
const FUEL_PASSIVE: f32 = 0.002; // drains always (life support)
const OXYGEN_DRAIN: f32 = 0.008; // drains always
const HULL_IMPACT_DMAGE: f32 = 18.0; // per hard landing / asteroid hit

// ── Planet constants ───────────────────────────────────────────────────────────
const PLANET_COUNT: usize = 18;
const MIN_PLANET_R: f32 = 120.0;
const MAX_PLANET_R: f32 = 340.0;
const MIN_PLANET_DIST: f32 = 3000.0; // min spacing between planets

// ── Asteroid constants ─────────────────────────────────────────────────────────
const ASTEROID_COUNT: usize = 35;
const ASTEROID_MIN_SIZE: f32 = 35.0;
const ASTEROID_MAX_SIZE: f32 = 110.0;

// ── Star field ─────────────────────────────────────────────────────────────────
const STAR_COUNT: usize = 200;

// ── Colours ────────────────────────────────────────────────────────────────────
const PLANET_COLORS: [(u8, u8, u8); 8] = [
    (80, 160, 255),  // ice blue
    (255, 120, 60),  // mars red
    (120, 220, 120), // jungle green
    (220, 200, 80),  // desert gold
    (180, 80, 220),  // purple gas giant
    (80, 220, 200),  // teal ocean
    (255, 160, 180), // pink
    (160, 200, 255), // pale blue
];

// ─────────────────────────────────────────────────────────────────────────────
// Simple deterministic pseudo-random (LCG) — no external crate needed
// ─────────────────────────────────────────────────────────────────────────────
fn lcg(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32) / (u32::MAX as f32)
}

fn lcg_range(seed: &mut u64, lo: f32, hi: f32) -> f32 {
    lo + lcg(seed) * (hi - lo)
}

// ─────────────────────────────────────────────────────────────────────────────
// Image helpers
// ─────────────────────────────────────────────────────────────────────────────
fn solid(r: u8, g: u8, b: u8, a: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(1, 1);
    img.put_pixel(0, 0, image::Rgba([r, g, b, a]));
    img
}

fn make_image(r: u8, g: u8, b: u8) -> Image {
    Image {
        shape: ShapeType::Rectangle(0.0, (1.0, 1.0), 0.0),
        image: solid(r, g, b, 255).into(),
        color: None,
    }
}

/// Draw a filled circle into an RgbaImage
fn circle_image(radius: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let d = radius * 2;
    let mut img = image::RgbaImage::new(d, d);
    let cx = radius as f32;
    let cy = radius as f32;
    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 - cx + 0.5;
            let dy = py as f32 - cy + 0.5;
            if dx * dx + dy * dy <= (radius as f32) * (radius as f32) {
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
    }
    img
}

/// Draw a circle with a lighter rim
fn planet_image(radius: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let d = radius * 2;
    let mut img = image::RgbaImage::new(d, d);
    let cx = radius as f32;
    let cy = radius as f32;
    let rf = radius as f32;
    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 - cx + 0.5;
            let dy = py as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= rf {
                let rim = ((rf - dist) / rf).min(1.0);
                let rr = (r as f32 * (0.7 + 0.3 * rim)).min(255.0) as u8;
                let gg = (g as f32 * (0.7 + 0.3 * rim)).min(255.0) as u8;
                let bb = (b as f32 * (0.7 + 0.3 * rim)).min(255.0) as u8;
                img.put_pixel(px, py, image::Rgba([rr, gg, bb, 255]));
            }
        }
    }
    img
}

/// Jagged asteroid polygon rasterised into an image
fn asteroid_image(size: u32, seed: u64) -> image::RgbaImage {
    let mut s = seed;
    let mut img = image::RgbaImage::new(size, size);
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let base_r = (size as f32 * 0.38).max(4.0);

    // build jagged radii at 12 angles
    let steps = 12usize;
    let radii: Vec<f32> = (0..steps)
        .map(|_| base_r * lcg_range(&mut s, 0.6, 1.0))
        .collect();

    for py in 0..size {
        for px in 0..size {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            let angle = dy.atan2(dx); // -pi..pi
            let norm = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI); // 0..1
            let idx = (norm * steps as f32) as usize % steps;
            let next = (idx + 1) % steps;
            let frac = (norm * steps as f32).fract();
            let r = radii[idx] * (1.0 - frac) + radii[next] * frac;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= r {
                img.put_pixel(px, py, image::Rgba([160, 140, 120, 255]));
            }
        }
    }
    img
}

/// Arrow / triangle ship image pointing upward
fn ship_image(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let cx = w as f32 / 2.0;
    for py in 0..h {
        for px in 0..w {
            let t = py as f32 / h as f32; // 0 (top/nose) .. 1 (bottom)
            let half_width = cx * t; // widens toward base
            let dx = (px as f32 - cx).abs();
            if dx < half_width {
                // engine glow at the base
                let (r, g, b) = if t > 0.80 {
                    (255, 160, 60)
                } else {
                    (200, 220, 255)
                };
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
    }
    img
}

/// Tiny star dot
fn star_image(brightness: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(3, 3);
    let b = brightness;
    img.put_pixel(1, 0, image::Rgba([b, b, b, 200]));
    img.put_pixel(0, 1, image::Rgba([b, b, b, 200]));
    img.put_pixel(1, 1, image::Rgba([b, b, b, 255]));
    img.put_pixel(2, 1, image::Rgba([b, b, b, 200]));
    img.put_pixel(1, 2, image::Rgba([b, b, b, 200]));
    img
}

/// Horizontal bar with fill ratio
fn bar_image(w: u32, h: u32, fill: f32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let filled = (w as f32 * fill.clamp(0.0, 1.0)) as u32;
    for py in 0..h {
        for px in 0..w {
            // border
            if px == 0 || px == w - 1 || py == 0 || py == h - 1 {
                img.put_pixel(px, py, image::Rgba([200, 200, 200, 255]));
            } else if px < filled {
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            } else {
                img.put_pixel(px, py, image::Rgba([30, 30, 40, 220]));
            }
        }
    }
    img
}

fn bar_to_image(w: u32, h: u32, fill: f32, r: u8, g: u8, b: u8) -> Image {
    Image {
        shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
        image: bar_image(w, h, fill, r, g, b).into(),
        color: None,
    }
}

/// Simple text label rendered as a white rectangle (placeholder)
/// In a real project wire up fontdue / prism text rendering.
fn text_image(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h {
        for px in 0..w {
            img.put_pixel(px, py, image::Rgba([255, 255, 255, 0]));
        }
    }
    img
}

// ─────────────────────────────────────────────────────────────────────────────
// Planet placement with min-distance enforcement
// ─────────────────────────────────────────────────────────────────────────────
struct PlanetSpec {
    x: f32,
    y: f32,
    radius: f32,
    color_idx: usize,
}

fn generate_planets(seed: &mut u64) -> Vec<PlanetSpec> {
    let mut planets: Vec<PlanetSpec> = Vec::new();

    // First planet very close to spawn so the player always has somewhere to land
    planets.push(PlanetSpec {
        x: VW / 2.0 + 1200.0,
        y: VH / 2.0 - 400.0,
        radius: 220.0,
        color_idx: 2, // green
    });

    let mut attempts = 0u32;
    while planets.len() < PLANET_COUNT && attempts < 5000 {
        attempts += 1;
        let x = lcg_range(seed, 1000.0, WORLD_SIZE - 1000.0);
        let y = lcg_range(seed, 1000.0, WORLD_SIZE - 1000.0);
        let r = lcg_range(seed, MIN_PLANET_R, MAX_PLANET_R);
        let col = (lcg(seed) * 8.0) as usize % 8;

        // Check min spacing
        let too_close = planets.iter().any(|p| {
            let dx = p.x - x;
            let dy = p.y - y;
            (dx * dx + dy * dy).sqrt() < MIN_PLANET_DIST
        });

        // Keep away from spawn
        let spawn_x = VW / 2.0;
        let spawn_y = VH / 2.0;
        let sdx = spawn_x - x;
        let sdy = spawn_y - y;
        let near_spawn = (sdx * sdx + sdy * sdy).sqrt() < 800.0;

        if !too_close && !near_spawn {
            planets.push(PlanetSpec { x, y, radius: r, color_idx: col });
        }
    }
    planets
}

// ─────────────────────────────────────────────────────────────────────────────
// Asteroid placement
// ─────────────────────────────────────────────────────────────────────────────
struct AsteroidSpec {
    x: f32,
    y: f32,
    size: f32,
    vx: f32,
    vy: f32,
    seed: u64,
}

fn generate_asteroids(seed: &mut u64, planets: &[PlanetSpec]) -> Vec<AsteroidSpec> {
    let mut asteroids = Vec::new();
    let mut attempts = 0u32;
    while asteroids.len() < ASTEROID_COUNT && attempts < 2000 {
        attempts += 1;
        let x = lcg_range(seed, 500.0, WORLD_SIZE - 500.0);
        let y = lcg_range(seed, 500.0, WORLD_SIZE - 500.0);
        let sz = lcg_range(seed, ASTEROID_MIN_SIZE, ASTEROID_MAX_SIZE);
        let vx = lcg_range(seed, -1.2, 1.2);
        let vy = lcg_range(seed, -1.2, 1.2);
        let aseed = (*seed) ^ 0xDEAD_BEEF;

        // keep away from planets
        let near_planet = planets.iter().any(|p| {
            let dx = p.x - x;
            let dy = p.y - y;
            (dx * dx + dy * dy).sqrt() < p.radius + 400.0
        });

        // keep away from spawn
        let sdx = VW / 2.0 - x;
        let sdy = VH / 2.0 - y;
        let near_spawn = (sdx * sdx + sdy * sdy).sqrt() < 1200.0;

        if !near_planet && !near_spawn {
            asteroids.push(AsteroidSpec { x, y, size: sz, vx, vy, seed: aseed });
        }
    }
    asteroids
}

// ─────────────────────────────────────────────────────────────────────────────
// Scene builders
// ─────────────────────────────────────────────────────────────────────────────

fn build_menu_scene(ctx: &mut Context) -> Scene {
    // Dark background panel
    let bg = GameObject::new_rect(
        ctx,
        "menu_bg".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: solid(5, 5, 20, 255).into(),
            color: None,
        }),
        (VW, VH),
        (0.0, 0.0),
        vec![],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // Title — rendered as a wide bright bar (text placeholder)
    let title = {
        // "LAST HORIZON" — write pixel letters manually (simplified large block)
        let w = 1400u32;
        let h = 220u32;
        let mut img = image::RgbaImage::new(w, h);
        // fill with a gradient cyan→white
        for py in 0..h {
            for px in 0..w {
                let t = px as f32 / w as f32;
                let r = (100.0 + 155.0 * t) as u8;
                let g = (200.0 + 55.0 * t) as u8;
                let b = 255u8;
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
        GameObject::new_rect(
            ctx,
            "menu_title".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.28),
            vec!["ui".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Sub-label bar
    let subtitle = {
        let w = 900u32;
        let h = 80u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                img.put_pixel(px, py, image::Rgba([160, 200, 255, 220]));
            }
        }
        GameObject::new_rect(
            ctx,
            "menu_sub".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.48),
            vec!["ui".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // START button
    let start_btn = {
        let w = 500u32;
        let h = 120u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let border = px == 0 || px == w - 1 || py == 0 || py == h - 1
                    || px == 1 || px == w - 2 || py == 1 || py == h - 2;
                let c = if border { 120u8 } else { 40u8 };
                img.put_pixel(px, py, image::Rgba([60, c, 180, 240]));
            }
        }
        GameObject::new_rect(
            ctx,
            "start_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(8.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.62),
            vec!["ui".into(), "button".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Decorative stars on menu
    let mut scene = Scene::new("menu")
        .with_object("menu_bg", bg)
        .with_object("menu_title", title)
        .with_object("menu_sub", subtitle)
        .with_object("start_btn", start_btn);

    let mut seed: u64 = 0xCAFEBABE;
    for i in 0..80usize {
        let x = lcg_range(&mut seed, 0.0, VW);
        let y = lcg_range(&mut seed, 0.0, VH);
        let br = lcg_range(&mut seed, 80.0, 255.0) as u8;
        let sz = if lcg(&mut seed) > 0.85 { 6.0 } else { 3.0 };
        let id = format!("mstar_{i}");
        let obj = GameObject::new_rect(
            ctx,
            id.clone().into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (sz, sz), 0.0),
                image: solid(br, br, br, 255).into(),
                color: None,
            }),
            (sz, sz),
            (x, y),
            vec![],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        scene = scene.with_object(id, obj);
    }

    // Space / click to start
    scene = scene
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
        );

    scene.on_enter(|canvas| {
        canvas.register_custom_event("goto_game".into(), |c| {
            c.load_scene("game");
        });
    })
}

// ─────────────────────────────────────────────────────────────────────────────

fn build_gameover_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(
        ctx,
        "go_bg".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: solid(10, 0, 0, 255).into(),
            color: None,
        }),
        (VW, VH),
        (0.0, 0.0),
        vec![],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // "MISSION FAILED" bar
    let title = {
        let w = 1200u32;
        let h = 200u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let t = py as f32 / h as f32;
                let r = 255u8;
                let g = (60.0 * (1.0 - t)) as u8;
                let b = 40u8;
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
        GameObject::new_rect(
            ctx,
            "go_title".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.25),
            vec!["ui".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Distance travelled label
    let dist_label = {
        let w = 700u32;
        let h = 90u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                img.put_pixel(px, py, image::Rgba([200, 200, 255, 200]));
            }
        }
        GameObject::new_rect(
            ctx,
            "go_dist".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.46),
            vec!["ui".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Retry button
    let retry_btn = {
        let w = 480u32;
        let h = 120u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let border = px == 0 || px == w - 1 || py == 0 || py == h - 1;
                let c = if border { 120u8 } else { 40u8 };
                img.put_pixel(px, py, image::Rgba([60, c, 180, 240]));
            }
        }
        GameObject::new_rect(
            ctx,
            "retry_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(8.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.60),
            vec!["ui".into(), "button".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Menu button
    let menu_btn = {
        let w = 480u32;
        let h = 120u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let border = px == 0 || px == w - 1 || py == 0 || py == h - 1;
                let c = if border { 100u8 } else { 20u8 };
                img.put_pixel(px, py, image::Rgba([c, 80, 160, 220]));
            }
        }
        GameObject::new_rect(
            ctx,
            "menu_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(8.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.74),
            vec!["ui".into(), "button".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    let scene = Scene::new("gameover")
        .with_object("go_bg", bg)
        .with_object("go_title", title)
        .with_object("go_dist", dist_label)
        .with_object("retry_btn", retry_btn)
        .with_object("menu_btn", menu_btn)
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
                target: Target::name("menu_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_btn"),
        );

    scene.on_enter(|canvas| {
        canvas.register_custom_event("go_retry".into(), |c| c.load_scene("game"));
        canvas.register_custom_event("go_menu".into(), |c| c.load_scene("menu"));
    })
}

// ─────────────────────────────────────────────────────────────────────────────

fn build_game_scene(ctx: &mut Context) -> Scene {
    let mut seed: u64 = 0x1234_5678_9ABC_DEF0;

    // ── Space background ───────────────────────────────────────────────────
    let space_bg = GameObject::new_rect(
        ctx,
        "space_bg".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (WORLD_SIZE, WORLD_SIZE), 0.0),
            image: solid(4, 4, 14, 255).into(),
            color: None,
        }),
        (WORLD_SIZE, WORLD_SIZE),
        (0.0, 0.0),
        vec![],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // ── Stars ──────────────────────────────────────────────────────────────
    let mut scene = Scene::new("game").with_object("space_bg", space_bg);

    for i in 0..STAR_COUNT {
        let x = lcg_range(&mut seed, 100.0, WORLD_SIZE - 100.0);
        let y = lcg_range(&mut seed, 100.0, WORLD_SIZE - 100.0);
        let br = lcg_range(&mut seed, 80.0, 255.0) as u8;
        let big = lcg(&mut seed) > 0.88;
        let sz = if big { 6.0 } else { 3.0 };
        let id = format!("star_{i}");
        let obj = GameObject::new_rect(
            ctx,
            id.clone().into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (sz, sz), 0.0),
                image: solid(br, br, br + 20, 255).into(),
                color: None,
            }),
            (sz, sz),
            (x, y),
            vec!["star".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        scene = scene.with_object(id, obj);
    }

    // ── Planets ────────────────────────────────────────────────────────────
    let planets = generate_planets(&mut seed);
    for (i, p) in planets.iter().enumerate() {
        let (r, g, b) = PLANET_COLORS[p.color_idx];
        let rad = p.radius as u32;
        let id = format!("planet_{i}");
        let obj = GameObject::new_rect(
            ctx,
            id.clone().into(),
            Some(Image {
                shape: ShapeType::Rectangle(p.radius, (p.radius * 2.0, p.radius * 2.0), 0.0),
                image: planet_image(rad, r, g, b).into(),
                color: None,
            }),
            (p.radius * 2.0, p.radius * 2.0),
            (p.x - p.radius, p.y - p.radius),
            vec!["planet".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
        .as_platform(); // treat as solid for collision
        scene = scene.with_object(id, obj);
    }

    // ── Asteroids ──────────────────────────────────────────────────────────
    let asteroids = generate_asteroids(&mut seed, &planets);
    for (i, a) in asteroids.iter().enumerate() {
        let sz = a.size as u32;
        let id = format!("asteroid_{i}");
        let obj = GameObject::new_rect(
            ctx,
            id.clone().into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (a.size, a.size), 0.0),
                image: asteroid_image(sz, a.seed).into(),
                color: None,
            }),
            (a.size, a.size),
            (a.x, a.y),
            vec!["asteroid".into()],
            (a.vx, a.vy),
            (1.0, 1.0), // no resistance — drifts forever
            0.0,
        );
        scene = scene.with_object(id, obj);
    }

    // ── Player ship ────────────────────────────────────────────────────────
    let spawn_x = WORLD_SIZE / 2.0 - SHIP_W / 2.0;
    let spawn_y = WORLD_SIZE / 2.0 - SHIP_H / 2.0;
    let player = GameObject::new_rect(
        ctx,
        "player".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (SHIP_W, SHIP_H), 0.0),
            image: ship_image(SHIP_W as u32, SHIP_H as u32).into(),
            color: None,
        }),
        (SHIP_W, SHIP_H),
        (spawn_x, spawn_y),
        vec!["player".into()],
        (0.0, 0.0),
        (1.0, 1.0), // space: no drag
        0.0,        // no gravity
    );
    scene = scene.with_object("player", player);

    // ── Thrust flame (child visual) ────────────────────────────────────────
    let flame = GameObject::new_rect(
        ctx,
        "flame".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (SHIP_W * 0.4, SHIP_H * 0.5), 0.0),
            image: solid(255, 140, 40, 200).into(),
            color: None,
        }),
        (SHIP_W * 0.4, SHIP_H * 0.5),
        (spawn_x + SHIP_W * 0.3, spawn_y + SHIP_H),
        vec!["flame".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );
    let mut flame_hidden = flame;
    flame_hidden.visible = false;
    scene = scene.with_object("flame", flame_hidden);

    // ── Landing indicator ──────────────────────────────────────────────────
    let land_ring = {
        let sz = 180u32;
        let mut img = image::RgbaImage::new(sz, sz);
        let cx = sz as f32 / 2.0;
        let cy = sz as f32 / 2.0;
        for py in 0..sz {
            for px in 0..sz {
                let dx = px as f32 - cx;
                let dy = py as f32 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d >= cx - 6.0 && d <= cx {
                    img.put_pixel(px, py, image::Rgba([80, 255, 120, 180]));
                }
            }
        }
        let mut obj = GameObject::new_rect(
            ctx,
            "land_ring".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (sz as f32, sz as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (sz as f32, sz as f32),
            (spawn_x - 50.0, spawn_y - 50.0),
            vec![],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        obj.visible = false;
        obj
    };
    scene = scene.with_object("land_ring", land_ring);

    // ── HUD ────────────────────────────────────────────────────────────────
    // Fuel bar
    let fuel_bar = GameObject::new_rect(
        ctx,
        "fuel_bar".into(),
        Some(bar_to_image(400, 36, 1.0, 80, 200, 255)),
        (400.0, 36.0),
        (60.0, 60.0),
        vec!["hud".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // Oxygen bar
    let oxy_bar = GameObject::new_rect(
        ctx,
        "oxy_bar".into(),
        Some(bar_to_image(400, 36, 1.0, 80, 255, 160)),
        (400.0, 36.0),
        (60.0, 118.0),
        vec!["hud".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // Hull bar
    let hull_bar = GameObject::new_rect(
        ctx,
        "hull_bar".into(),
        Some(bar_to_image(400, 36, 1.0, 255, 120, 80)),
        (400.0, 36.0),
        (60.0, 176.0),
        vec!["hud".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );

    // Distance counter (thin bar used as indicator background)
    let dist_bar = {
        let w = 300u32;
        let h = 50u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let border = px == 0 || px == w - 1 || py == 0 || py == h - 1;
                img.put_pixel(
                    px,
                    py,
                    image::Rgba(if border {
                        [160, 160, 200, 200]
                    } else {
                        [20, 20, 35, 180]
                    }),
                );
            }
        }
        GameObject::new_rect(
            ctx,
            "dist_bar".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW - 380.0, 60.0),
            vec!["hud".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };

    // Small label dots next to bars (coloured markers)
    fn make_label(ctx: &mut Context, id: &str, r: u8, g: u8, b: u8, y: f32) -> GameObject {
        GameObject::new_rect(
            ctx,
            id.to_string().into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (40.0, 32.0), 0.0),
                image: solid(r, g, b, 255).into(),
                color: None,
            }),
            (40.0, 32.0),
            (14.0, y),
            vec!["hud".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    }

    let fuel_lbl = make_label(ctx, "fuel_lbl", 80, 200, 255, 64.0);
    let oxy_lbl  = make_label(ctx, "oxy_lbl",  80, 255, 160, 122.0);
    let hull_lbl = make_label(ctx, "hull_lbl", 255, 120, 80,  180.0);

    // "LANDED" flash
    let landed_flash = {
        let w = 600u32;
        let h = 100u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                img.put_pixel(px, py, image::Rgba([80, 255, 120, 230]));
            }
        }
        let mut obj = GameObject::new_rect(
            ctx,
            "landed_flash".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.42),
            vec!["hud".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        obj.visible = false;
        obj
    };

    // "LOW FUEL" warning
    let low_fuel_warn = {
        let w = 500u32;
        let h = 80u32;
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                img.put_pixel(px, py, image::Rgba([255, 200, 40, 230]));
            }
        }
        let mut obj = GameObject::new_rect(
            ctx,
            "low_fuel_warn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (w as f32, h as f32),
            (VW / 2.0 - w as f32 / 2.0, VH * 0.08),
            vec!["hud".into()],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        );
        obj.visible = false;
        obj
    };

    scene = scene
        .with_object("fuel_bar", fuel_bar)
        .with_object("oxy_bar", oxy_bar)
        .with_object("hull_bar", hull_bar)
        .with_object("dist_bar", dist_bar)
        .with_object("fuel_lbl", fuel_lbl)
        .with_object("oxy_lbl", oxy_lbl)
        .with_object("hull_lbl", hull_lbl)
        .with_object("landed_flash", landed_flash)
        .with_object("low_fuel_warn", low_fuel_warn);

    // ── Key events ─────────────────────────────────────────────────────────
    // Thrust
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Named(NamedKey::ArrowUp),
            action: Action::Custom { name: "thrust".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    // Also W
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Character("w".into()),
            action: Action::Custom { name: "thrust".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    // Rotate left
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Named(NamedKey::ArrowLeft),
            action: Action::Custom { name: "rotate_left".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Character("a".into()),
            action: Action::Custom { name: "rotate_left".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    // Rotate right
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Named(NamedKey::ArrowRight),
            action: Action::Custom { name: "rotate_right".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Character("d".into()),
            action: Action::Custom { name: "rotate_right".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    // Brake
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Named(NamedKey::ArrowDown),
            action: Action::Custom { name: "brake".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );
    scene = scene.with_event(
        GameEvent::KeyHold {
            key: Key::Character("s".into()),
            action: Action::Custom { name: "brake".into() },
            target: Target::name("player"),
        },
        Target::name("player"),
    );

    // ── on_enter: game logic ────────────────────────────────────────────────
    scene.on_enter(|canvas| {
        // Camera
        let mut cam = Camera::new((WORLD_SIZE, WORLD_SIZE), (VW, VH));
        cam.follow(Some(Target::name("player")));
        cam.lerp_speed = 0.12;
        canvas.set_camera(cam);

        // ── Shared mutable state ──────────────────────────────────────────
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct State {
            angle_deg: f32,   // ship heading (0 = up, clockwise)
            fuel:      f32,   // 0..100
            oxygen:    f32,   // 0..100
            hull:      f32,   // 0..100
            thrusting: bool,
            landed_on: Option<usize>, // planet index currently landed on
            landed_ticks: u32,
            distance: f32,   // total distance flown
            spawn_x: f32,
            spawn_y: f32,
            dead: bool,
            warn_tick: u32,
            low_fuel_visible: bool,
        }

        let state = Arc::new(Mutex::new(State {
            angle_deg: 0.0,
            fuel: 100.0,
            oxygen: 100.0,
            hull: 100.0,
            thrusting: false,
            landed_on: None,
            landed_ticks: 0,
            distance: 0.0,
            spawn_x: WORLD_SIZE / 2.0,
            spawn_y: WORLD_SIZE / 2.0,
            dead: false,
            warn_tick: 0,
            low_fuel_visible: false,
        }));

        // ── Rotation ──────────────────────────────────────────────────────
        let st = state.clone();
        canvas.register_custom_event("rotate_left".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.landed_on.is_some() { return; }
            s.angle_deg -= ROTATION_SPEED;
            // Rotate ship image by redrawing (simple approach: tint)
            // For a real rotation, prism would need transform support.
            // We approximate by swapping the momentum direction.
            drop(s);
        });

        let st = state.clone();
        canvas.register_custom_event("rotate_right".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.landed_on.is_some() { return; }
            s.angle_deg += ROTATION_SPEED;
            drop(s);
        });

        // ── Thrust ────────────────────────────────────────────────────────
        let st = state.clone();
        canvas.register_custom_event("thrust".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.landed_on.is_some() { return; }
            if s.fuel <= 0.0 { return; }
            let angle_rad = s.angle_deg.to_radians();
            // "up" direction rotated by angle
            let ax = angle_rad.sin() * THRUST_FORCE;
            let ay = -angle_rad.cos() * THRUST_FORCE;
            s.fuel -= FUEL_DRAIN;
            s.thrusting = true;
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum.0 = (obj.momentum.0 + ax).clamp(-MAX_SPEED, MAX_SPEED);
                obj.momentum.1 = (obj.momentum.1 + ay).clamp(-MAX_SPEED, MAX_SPEED);
            }
        });

        // ── Brake ─────────────────────────────────────────────────────────
        let st2 = state.clone();
        canvas.register_custom_event("brake".into(), move |c| {
            let s = st2.lock().unwrap();
            if s.landed_on.is_some() { return; }
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum.0 *= 0.90;
                obj.momentum.1 *= 0.90;
            }
        });

        // ── Main tick ─────────────────────────────────────────────────────
        let st = state.clone();
        canvas.on_update(move |c| {
            let mut s = st.lock().unwrap();
            if s.dead { return; }

            // Reset thrust flag each tick (set true only when thrust fires)
            s.thrusting = false;

            // Get player state
            let (px, py, vx, vy) = c.get_game_object("player")
                .map(|p| (p.position.0, p.position.1, p.momentum.0, p.momentum.1))
                .unwrap_or((WORLD_SIZE / 2.0, WORLD_SIZE / 2.0, 0.0, 0.0));

            let speed = (vx * vx + vy * vy).sqrt();

            // Distance from spawn
            let dx = px - s.spawn_x;
            let dy = py - s.spawn_y;
            let d_from_spawn = (dx * dx + dy * dy).sqrt();
            if d_from_spawn > s.distance {
                s.distance = d_from_spawn;
            }

            // Passive resource drains
            if s.landed_on.is_none() {
                s.oxygen -= OXYGEN_DRAIN;
                s.fuel   -= FUEL_PASSIVE;
            }

            // Clamp resources
            s.fuel    = s.fuel.max(0.0);
            s.oxygen  = s.oxygen.max(0.0);
            s.hull    = s.hull.max(0.0);

            // ── Planet proximity / landing ────────────────────────────────
            let ship_cx = px + SHIP_W / 2.0;
            let ship_cy = py + SHIP_H / 2.0;

            let mut near_planet = false;
            let mut landing_planet: Option<usize> = None;

            for i in 0..PLANET_COUNT {
                let pname = format!("planet_{i}");
                if let Some(planet) = c.get_game_object(&pname) {
                    let pcx = planet.position.0 + planet.size.0 / 2.0;
                    let pcy = planet.position.1 + planet.size.1 / 2.0;
                    let prad = planet.size.0 / 2.0;

                    let dist_to_center = {
                        let ddx = ship_cx - pcx;
                        let ddy = ship_cy - pcy;
                        (ddx * ddx + ddy * ddy).sqrt()
                    };

                    let approach_dist = prad + SHIP_W * 1.8;
                    let land_dist     = prad + SHIP_W * 0.55;

                    if dist_to_center < approach_dist {
                        near_planet = true;
                    }

                    if dist_to_center < land_dist {
                        landing_planet = Some(i);
                    }
                }
            }

            // Show/hide landing ring
            if let Some(obj) = c.get_game_object_mut("land_ring") {
                obj.visible = near_planet && s.landed_on.is_none();
                obj.position = (px - 50.0, py - 50.0);
            }

            // Flame position tracking (follows ship)
            let angle_rad = s.angle_deg.to_radians();
            let flame_ox = -angle_rad.sin() * SHIP_H * 0.5 - (SHIP_W * 0.2);
            let flame_oy =  angle_rad.cos() * SHIP_H * 0.5;
            if let Some(flame) = c.get_game_object_mut("flame") {
                flame.position = (px + SHIP_W / 2.0 + flame_ox, py + SHIP_H / 2.0 + flame_oy);
            }

            // ── Landing logic ─────────────────────────────────────────────
            if let Some(planet_idx) = landing_planet {
                if s.landed_on.is_none() {
                    if speed < SAFE_LAND_SPEED {
                        // Safe landing
                        s.landed_on = Some(planet_idx);
                        s.landed_ticks = 0;
                        if let Some(obj) = c.get_game_object_mut("player") {
                            obj.momentum = (0.0, 0.0);
                        }
                        if let Some(flash) = c.get_game_object_mut("landed_flash") {
                            flash.visible = true;
                        }
                    } else {
                        // Hard landing — hull damage
                        let dmg = ((speed - SAFE_LAND_SPEED) / MAX_SPEED * HULL_IMPACT_DMAGE * 2.0).min(HULL_IMPACT_DMAGE);
                        s.hull -= dmg;
                        // Bounce back
                        if let Some(obj) = c.get_game_object_mut("player") {
                            obj.momentum.0 *= -0.5;
                            obj.momentum.1 *= -0.5;
                        }
                    }
                }
            } else {
                s.landed_on = None;
            }

            // While landed: refuel + reoxygenate
            if s.landed_on.is_some() {
                s.landed_ticks += 1;
                s.fuel   = (s.fuel   + 0.25).min(100.0);
                s.oxygen = (s.oxygen + 0.20).min(100.0);
                s.hull   = (s.hull   + 0.06).min(100.0);

                // Hide flash after 60 ticks
                if s.landed_ticks == 60 {
                    if let Some(flash) = c.get_game_object_mut("landed_flash") {
                        flash.visible = false;
                    }
                }
            }

            // ── Asteroid collision ────────────────────────────────────────
            for i in 0..ASTEROID_COUNT {
                let aname = format!("asteroid_{i}");
                if let Some(ast) = c.get_game_object(&aname) {
                    // Wrap asteroid around world edges
                    let ax = ast.position.0;
                    let ay = ast.position.1;
                    let asz = ast.size.0;
                    drop(ast);

                    let mut new_ax = ax;
                    let mut new_ay = ay;
                    if ax > WORLD_SIZE { new_ax = -asz; }
                    if ax + asz < 0.0  { new_ax = WORLD_SIZE; }
                    if ay > WORLD_SIZE { new_ay = -asz; }
                    if ay + asz < 0.0  { new_ay = WORLD_SIZE; }
                    if new_ax != ax || new_ay != ay {
                        if let Some(ast_mut) = c.get_game_object_mut(&aname) {
                            ast_mut.position = (new_ax, new_ay);
                        }
                    }

                    // AABB collision with player
                    if let Some(ast) = c.get_game_object(&aname) {
                        let overlap = px < ast.position.0 + ast.size.0
                            && px + SHIP_W > ast.position.0
                            && py < ast.position.1 + ast.size.1
                            && py + SHIP_H > ast.position.1;
                        if overlap && s.landed_on.is_none() {
                            s.hull -= HULL_IMPACT_DMAGE * 0.5;
                            // Knock ship away
                            if let Some(obj) = c.get_game_object_mut("player") {
                                obj.momentum.0 = (obj.momentum.0 * -1.2).clamp(-MAX_SPEED, MAX_SPEED);
                                obj.momentum.1 = (obj.momentum.1 * -1.2).clamp(-MAX_SPEED, MAX_SPEED);
                            }
                        }
                    }
                }
            }

            // ── Low fuel warning ──────────────────────────────────────────
            s.warn_tick = s.warn_tick.wrapping_add(1);
            let should_warn = s.fuel < 20.0 && (s.warn_tick / 30) % 2 == 0;
            if should_warn != s.low_fuel_visible {
                s.low_fuel_visible = should_warn;
                if let Some(obj) = c.get_game_object_mut("low_fuel_warn") {
                    obj.visible = should_warn;
                }
            }

            // ── Update HUD bars ───────────────────────────────────────────
            let fuel_fill  = s.fuel   / 100.0;
            let oxy_fill   = s.oxygen / 100.0;
            let hull_fill  = s.hull   / 100.0;

            if let Some(obj) = c.get_game_object_mut("fuel_bar") {
                let r = if s.fuel < 20.0 { 255 } else { 80 };
                let g = if s.fuel < 20.0 { 80  } else { 200 };
                obj.set_image(bar_to_image(400, 36, fuel_fill, r, g, 255));
            }
            if let Some(obj) = c.get_game_object_mut("oxy_bar") {
                let r = if s.oxygen < 20.0 { 255 } else { 80 };
                let g = if s.oxygen < 20.0 { 80  } else { 255 };
                obj.set_image(bar_to_image(400, 36, oxy_fill, r, g, 160));
            }
            if let Some(obj) = c.get_game_object_mut("hull_bar") {
                let r = if s.hull < 30.0 { 255 } else { 255 };
                let g = if s.hull < 30.0 { 40  } else { 120 };
                obj.set_image(bar_to_image(400, 36, hull_fill, r, g, 80));
            }

            // ── Death check ───────────────────────────────────────────────
            if s.oxygen <= 0.0 || s.hull <= 0.0 || s.fuel <= 0.0 {
                s.dead = true;
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
