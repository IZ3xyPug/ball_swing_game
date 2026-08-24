// ── scenes/game/fx.rs — mega-shader / FX plumbing ─────────────────────────────
// The engine already exposes `register_shader_source` and `push_mega_sprite`.
// These thin helpers make that wiring usable from game logic. The WGSL shader
// sources themselves are authored separately (see `wgpu_canvas`'s
// `renderer/mega_shader/common_effects.wgsl` for a reference); call
// `register_mega_shader` once at scene setup before any `push_mega_fx`.

use quartz::*;
use std::sync::Arc;

/// Register a named WGSL mega-shader source. Emitted as a `RegisterShader`
/// envelope on the next draw; must be done before pushing sprites that use it.
pub fn register_mega_shader(c: &mut Canvas, id: &str, label: &str, wgsl: &str) {
    c.register_shader_source(id, label, wgsl);
}

/// Queue a mega-shader sprite for this frame.
///
/// - `image` — the diffuse texture (an `Arc<RgbaImage>`).
/// - `pos` — world-space centre.
/// - `scale` — world-space width/height.
/// - `tint` — RGBA multiplier.
/// - `variant` — `0` = common effects, `1` = animated VFX.
///
/// Sprites are cleared after each frame's draw, so call this every frame.
pub fn push_mega_fx(
    c: &mut Canvas,
    image: Arc<image::RgbaImage>,
    pos: (f32, f32),
    scale: (f32, f32),
    tint: (f32, f32, f32, f32),
    variant: u32,
) {
    let sprite = MegaShaderSprite {
        image,
        instance: MegaShaderInstance {
            world_position: pos,
            scale,
            rotation: 0.0,
            tint_color: tint,
            bitmask: [0; 4],
            velocity: (0.0, 0.0),
        },
        shader_variant: variant,
    };
    c.push_mega_sprite(sprite);
}

/// Convenience: a plain white 1×1 texture for pure-effect mega sprites.
pub fn flat_white() -> Arc<image::RgbaImage> {
    Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])))
}
