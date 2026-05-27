// ── objects/gravity_cannon.rs ─────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;

pub fn make_gravity_cannon(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    // Decode frame 8 (last frame) from the GIF as the default resting image.
    let default_img: std::sync::Arc<image::RgbaImage> = {
        use image::{AnimationDecoder, imageops};
        let cursor = std::io::Cursor::new(ASSET_GRAVITYCANNON_GIF);
        let maybe = (|| -> Option<std::sync::Arc<image::RgbaImage>> {
            let decoder = image::codecs::gif::GifDecoder::new(cursor).ok()?;
            let frames = decoder.into_frames().collect_frames().ok()?;
            let raw = frames.into_iter().last()?;
            let scaled = imageops::resize(
                raw.buffer(),
                GRAVITYCANNON_W as u32,
                GRAVITYCANNON_H as u32,
                imageops::FilterType::Nearest,
            );
            Some(std::sync::Arc::new(scaled))
        })();
        maybe.unwrap_or_else(|| {
            std::sync::Arc::new(image::RgbaImage::new(
                GRAVITYCANNON_W as u32,
                GRAVITYCANNON_H as u32,
            ))
        })
    };

    let mut obj = GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (GRAVITYCANNON_W, GRAVITYCANNON_H), 0.0),
            image: default_img,
            color: None,
        }),
        (GRAVITYCANNON_W, GRAVITYCANNON_H),
        (x - GRAVITYCANNON_W * 0.5, y - GRAVITYCANNON_H * 0.5),
        vec!["gravity_cannon".into()],
        (0.0, 0.0),   // momentum
        (1.0, 1.0),   // resistance
        0.0,          // gravity — cannon floats manually, never physics-driven
    );
    obj.rotation = CANNON_DEFAULT_ROTATION;
    obj.layer = 30; // normal layer — elevated to LAYER_CANNON_ACTIVE during capture
    obj
}
