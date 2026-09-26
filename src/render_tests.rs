//! Game sprites rendered through the REAL engine path — GameObject draw ->
//! the GPU — and compared with their source art.
//!
//! Written for "the generators look dark, not like the image you showed me":
//! a sprite that renders darker than its PNG is invisible to every test that
//! checks game state, and only shows on a screen.

use crate::constants::*;
use prism::canvas::{Area, Item};
use prism::drawable::Drawable;
use quartz::{Color, GameObject, GlowConfig, Image, ShapeType};

const W: u32 = 256;
const H: u32 = 256;

/// A generator built the way bootstrap builds one.
fn generator(glow: bool) -> GameObject {
    let (mut ctx, _rx) = prism::Context::new();
    let (intact, _, idle) = devourer_generator_assets();
    let d = 190.0f32;
    let img = crate::scenes::game::helpers::pl_image_cached(intact, d).expect("generator art decodes");
    let mut g = GameObject::new_rect(
        &mut ctx, "gen".into(),
        Some(Image { shape: ShapeType::Ellipse(0.0, (d, d), 0.0), image: img, color: None }),
        (d, d), (33.0, 33.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    g.gravity = 0.0;
    g.animated_sprite = crate::scenes::game::helpers::pl_sprite(idle, d, DEVOURER_GENERATOR_IDLE_FPS);
    if glow {
        g.set_glow(GlowConfig { color: Color(C_BOSS_GENERATOR.0, C_BOSS_GENERATOR.1, C_BOSS_GENERATOR.2, 200), width: 10.0 });
    }
    g.update_animation(0.0);
    g
}

/// Mean brightness of the generator's own pixels (where its first idle
/// frame is opaque), as rendered.
fn rendered_mean(g: &GameObject, lighting: Option<f32>) -> Option<f32> {
    let mut r = pollster::block_on(prism::canvas::HeadlessRenderer::new(W, H)).ok()?;
    let req = g.request_size();
    let sized = g.build((W as f32, H as f32), req);
    let mut items: Vec<(Area, Item)> = Vec::new();
    if let Some(ambient) = lighting {
        items.push((Area { offset: (0.0, 0.0), bounds: None }, Item::SetLights {
            ambient_rgb: (1.0, 1.0, 1.0), ambient_strength: ambient, lights: vec![], occluders: vec![],
        }));
    }
    items.extend(g.draw(&sized, (33.0, 33.0), (0.0, 0.0, W as f32, H as f32)));
    let (_, _, rgba) = r.render(items);

    let frame = crate::scenes::game::helpers::pl_image_cached(devourer_generator_assets().2[0], 190.0).unwrap();
    let (mut sum, mut n) = (0.0f32, 0u32);
    for y in 0..190u32 {
        for x in 0..190u32 {
            if frame.get_pixel(x, y)[3] == 255 {
                let i = (((y + 33) * W + (x + 33)) * 4) as usize;
                sum += (rgba[i] as f32 + rgba[i + 1] as f32 + rgba[i + 2] as f32) / 3.0;
                n += 1;
            }
        }
    }
    Some(sum / n.max(1) as f32)
}

/// The same pixels in the source art.
fn source_mean() -> f32 {
    let frame = crate::scenes::game::helpers::pl_image_cached(devourer_generator_assets().2[0], 190.0).unwrap();
    let (mut sum, mut n) = (0.0f32, 0u32);
    for p in frame.pixels().filter(|p| p[3] == 255) {
        sum += (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0;
        n += 1;
    }
    sum / n.max(1) as f32
}

#[test]
fn a_generator_renders_as_bright_as_its_art() {
    let src = source_mean();
    let cases = [
        ("no lighting, no glow", generator(false), None),
        ("no lighting, glow", generator(true), None),
        ("full ambient, glow", generator(true), Some(1.0)),
    ];
    let mut report = String::new();
    let mut ok = true;
    for (name, g, lighting) in &cases {
        let Some(m) = rendered_mean(g, *lighting) else {
            eprintln!("no GPU available; skipping");
            return;
        };
        report.push_str(&format!("  {name}: {m:.1} (art {src:.1})\n"));
        if (m - src).abs() > src * 0.12 {
            ok = false;
        }
    }
    eprintln!("generator brightness:\n{report}");
    assert!(ok, "a generator renders noticeably darker or brighter than its art:\n{report}");
}
