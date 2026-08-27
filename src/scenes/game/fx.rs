// ── scenes/game/fx.rs — mega-shader / FX plumbing ─────────────────────────────
// The engine already exposes `register_shader_source` and `push_mega_sprite`.
// These thin helpers make that wiring usable from game logic. The WGSL shader
// sources themselves are authored separately (see `wgpu_canvas`'s
// `renderer/mega_shader/common_effects.wgsl` for a reference); call
// `register_mega_shader` once at scene setup before any `push_mega_fx`.

use quartz::*;
use std::sync::Arc;
use crate::constants::*;

/// Register a named WGSL mega-shader source. Emitted as a `RegisterShader`
/// envelope on the next draw; must be done before pushing sprites that use it.
pub fn register_mega_shader(c: &mut Canvas, id: &str, label: &str, wgsl: &str) {
    c.register_shader_source(id, label, wgsl);
}

/// Convert a world-space centre + size into the UV (0..1) screen space the mega
/// shader renderer projects sprites in. The renderer's `prepare` uses a fixed
/// `ortho(0,1)` camera, so sprites must be submitted in UV coordinates; this
/// maps a world position through the active scene camera. Falls back to passing
/// the values through when no camera is active.
pub fn world_to_mega_uv(
    c: &Canvas,
    pos: (f32, f32),
    scale: (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    if let Some(cam) = c.camera() {
        let z = cam.zoom.max(0.01);
        let (sx, sy) = cam.world_to_screen(pos);
        ((sx / VW, sy / VH), (scale.0 * z / VW, scale.1 * z / VH))
    } else {
        (pos, scale)
    }
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
    let ((u, v), (su, sv)) = world_to_mega_uv(c, pos, scale);
    let sprite = MegaShaderSprite {
        image,
        instance: MegaShaderInstance {
            world_position: (u, v),
            scale: (su, sv),
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

/// Push an animated-VFX **electricity** sprite over a world position/scale.
/// Used as the "electricity ball" effect on the player while the boss buff is
/// active. `shader_variant = 1` (animated VFX) with the `BIT_ELECTRICITY` bit.
/// Sprites are cleared each frame, so call this every frame while the effect
/// should be visible.
pub fn push_electric_fx(
    c: &mut Canvas,
    pos: (f32, f32),
    scale: (f32, f32),
    tint: (f32, f32, f32, f32),
) {
    let ((u, v), (su, sv)) = world_to_mega_uv(c, pos, scale);
    let sprite = MegaShaderSprite {
        image: flat_white(),
        instance: MegaShaderInstance {
            world_position: (u, v),
            scale: (su, sv),
            rotation: 0.0,
            tint_color: tint,
            // BIT_ELECTRICITY = 1 << 2 (see animated_vfx.wgsl).
            bitmask: [1 << 2, 0, 0, 0],
            velocity: (0.0, 0.0),
        },
        shader_variant: 1,
    };
    c.push_mega_sprite(sprite);
}

/// Push a full spherical energy dome centred on `pos`.
///
/// Uses `BIT_ENERGY_DOME` rather than `BIT_AIR_SHIELD`: the air shield is a
/// forward-facing arc keyed to velocity and reads as speed, while a player
/// sheltering from a solar flare needs cover that visibly surrounds them.
/// `tint` is (r, g, b, strength).
pub fn push_energy_dome_fx(
    c: &mut Canvas,
    pos: (f32, f32),
    scale: (f32, f32),
    tint: (f32, f32, f32, f32),
) {
    let ((u, v), (su, sv)) = world_to_mega_uv(c, pos, scale);
    let sprite = MegaShaderSprite {
        image: flat_white(),
        instance: MegaShaderInstance {
            world_position: (u, v),
            scale: (su, sv),
            rotation: 0.0,
            tint_color: tint,
            bitmask: [MEGA_BIT_ENERGY_DOME, 0, 0, 0],
            velocity: (0.0, 0.0),
        },
        shader_variant: 1,
    };
    c.push_mega_sprite(sprite);
}
