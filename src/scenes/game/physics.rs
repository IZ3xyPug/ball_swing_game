use quartz::*;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};

use image::AnimationDecoder;

use crate::constants::*;
use crate::state::*;

static ROPE_FX_FRAMES: OnceLock<Vec<Arc<image::RgbaImage>>> = OnceLock::new();
static ROPE_FX_CACHE: OnceLock<Mutex<HashMap<(usize, u32, u32), Arc<image::RgbaImage>>>> = OnceLock::new();
const ROPE_FX_SUPERSAMPLE: u32 = 1;
const ROPE_LEN_QUANTUM_PX: u32 = 20;
// How far the rope image extends past the hook center (away from player).
// Enough to hide cleanly behind the hook sprite but not protrude past it.
const ROPE_HOOK_PAD:   f32 = 62.0; // HOOK_R(38) + 24
// How far the rope image extends past the player center.
// Must cover PLAYER_R + look-ahead motion at max swing speed.
const ROPE_PLAYER_PAD: f32 = PLAYER_R + 35.0; // 75px

fn quantize_len_px(len_px: u32) -> u32 {
    let q = ROPE_LEN_QUANTUM_PX.max(1);
    ((len_px + q / 2) / q) * q
}

fn rope_fx_frames() -> &'static Vec<Arc<image::RgbaImage>> {
    ROPE_FX_FRAMES.get_or_init(|| {
        let cursor = Cursor::new(include_bytes!("../../../assets/energy_hook_1.gif").as_slice());
        if let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) {
            let mut frames: Vec<Arc<image::RgbaImage>> = Vec::new();
            for frame_result in decoder.into_frames() {
                if let Ok(frame) = frame_result {
                    frames.push(Arc::new(frame.into_buffer()));
                }
            }
            if !frames.is_empty() {
                return frames;
            }
        }
        vec![Arc::new(image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 255, 255, 255])))]
    })
}

/// Builds the rope image from the GIF itself (no tiling), stretched along
/// the GIF's vertical axis: width -> beam thickness, height -> rope length.
fn rope_fx_image(frame_idx: usize, rope_len_px: u32, beam_px: u32) -> Arc<image::RgbaImage> {
    let frames = rope_fx_frames();
    let idx = frame_idx % frames.len().max(1);
    let len_px = quantize_len_px(rope_len_px.max(1)).max(1);
    let thick_px = beam_px.max(1);
    let key = (idx, len_px, thick_px);

    let cache = ROPE_FX_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(img) = cache.lock().unwrap().get(&key).cloned() {
        return img;
    }

    let ss = ROPE_FX_SUPERSAMPLE.max(1);
    let target_w = thick_px.saturating_mul(ss);
    let target_h = len_px.saturating_mul(ss);
    let src = frames[idx].as_ref();
    let resized = image::imageops::resize(src, target_w, target_h, image::imageops::FilterType::Nearest);
    let out = Arc::new(resized);

    let mut cache_guard = cache.lock().unwrap();
    if cache_guard.len() > 800 {
        cache_guard.clear();
    }
    cache_guard.insert(key, out.clone());
    out
}

/// Spawn a background thread to pre-generate and cache all rope textures.
/// Uses FilterType::Nearest so each texture takes <1ms even in debug builds,
/// completing the full cache in well under a second before the player can grab.
pub fn prewarm_rope_fx_cache() {
    let beam_px = ROPE_THICKNESS.round().max(2.0) as u32;
    let n_frames = rope_fx_frames().len();
    const VEL_LOOK: f32 = 1.0;
    let min_draw = (ROPE_LEN_MIN + ROPE_PLAYER_PAD).max(1.0);
    let max_draw = ROPE_LEN_MAX + ROPE_PLAYER_PAD + MOMENTUM_CAP * VEL_LOOK;
    let min_q = quantize_len_px(min_draw as u32 - ROPE_LEN_QUANTUM_PX);
    let max_q = quantize_len_px(max_draw as u32 + ROPE_LEN_QUANTUM_PX);
    let step  = ROPE_LEN_QUANTUM_PX.max(1);
    std::thread::spawn(move || {
        let mut len = min_q;
        while len <= max_q {
            for frame_idx in 0..n_frames {
                rope_fx_image(frame_idx, len, beam_px);
            }
            len += step;
        }
    });
}

/// Sync player position/velocity from engine object into State.
/// Call at the start of each tick before any game logic.
pub fn read_player_from_engine(c: &mut Canvas, s: &mut State) {
    if let Some(obj) = c.get_game_object("player") {
        s.px = obj.position.0 + PLAYER_R;
        s.py = obj.position.1 + PLAYER_R;
        s.vx = obj.momentum.0;
        s.vy = obj.momentum.1;
    }
}

/// Apply rope constraint when hooked. Modifies State velocity/position and
/// updates the rope visual. Also sets engine gravity to 0 (tangential gravity
/// is applied manually inside the constraint).
pub fn tick_rope_constraint(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.hooked {
        drop(s);
        if let Some(obj) = c.get_game_object_mut("rope") {
            obj.visible = false;
        }
        return;
    }

    let dx   = s.px - s.hook_x;
    let dy   = s.py - s.hook_y;
    let dist = (dx*dx + dy*dy).sqrt().max(1.0);
    let nx = dx / dist;
    let ny = dy / dist;
    let tx = -ny;
    let ty = nx;

    let radial_v = s.vx * nx + s.vy * ny;
    let mut tangent_v = s.vx * tx + s.vy * ty;
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };

    // Project position onto arc and strip radial velocity.
    s.px = s.hook_x + nx * s.rope_len;
    s.py = s.hook_y + ny * s.rope_len;
    s.vx -= radial_v * nx * SWING_TENSION;
    s.vy -= radial_v * ny * SWING_TENSION;

    // In space, preserve tangential velocity exactly so rotation direction and
    // speed stay constant while hooked. Normal mode keeps gravity + drag.
    if !s.in_space_mode {
        tangent_v += GRAVITY * gravity_scale * s.gravity_dir * ty;
        tangent_v *= SWING_DRAG;
    }
    s.vx = tx * tangent_v;
    s.vy = ty * tangent_v;

    // Update rope visual.
    let (rdx, rdy, hx, hy) = (s.px - s.hook_x, s.py - s.hook_y, s.hook_x, s.hook_y);
    let rope_tick = s.ticks;
    let rope_vx = s.vx;
    let rope_vy = s.vy;
    drop(s);

    // Velocity look-ahead: the engine applies obj.momentum to obj.position AFTER
    // on_update, so the rendered player is 1 frame ahead of state. Project the
    // player end forward by that amount so the rope tracks the ball at high speed.
    const VEL_LOOK: f32 = 1.0;
    let vis_px = hx + rdx + rope_vx * VEL_LOOK;
    let vis_py = hy + rdy + rope_vy * VEL_LOOK;
    let vis_rdx = vis_px - hx;
    let vis_rdy = vis_py - hy;
    let vis_dist = (vis_rdx * vis_rdx + vis_rdy * vis_rdy).sqrt().max(1.0);
    let unit_x = vis_rdx / vis_dist;
    let unit_y = vis_rdy / vis_dist;
    let rope_ang = vis_rdy.atan2(vis_rdx).to_degrees();

    // Rope draws from hook CENTER so the rectangle extends into the hook sprite,
    // filling the gap. The far end extends ROPE_PLAYER_PAD past the player center.
    let end_x = vis_px + unit_x * ROPE_PLAYER_PAD;
    let end_y = vis_py + unit_y * ROPE_PLAYER_PAD;
    let seg_dx = end_x - hx;
    let seg_dy = end_y - hy;
    let rope_draw_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt().max(1.0);
    let rope_mid_x = (hx + end_x) * 0.5;
    let rope_mid_y = (hy + end_y) * 0.5;

    let rope_beam = ROPE_THICKNESS.max(2.0);
    let rope_beam_px = rope_beam.round().max(2.0) as u32;
    let rope_len_px = rope_draw_len.round().max(1.0) as u32;
    let frame_idx = ((rope_tick as usize) / 2) % rope_fx_frames().len().max(1);
    let rope_img = rope_fx_image(frame_idx, rope_len_px, rope_beam_px);

    if let Some(rope_obj) = c.get_game_object_mut("rope") {
        rope_obj.size = (rope_beam, rope_draw_len);
        rope_obj.position = (rope_mid_x - rope_beam * 0.5, rope_mid_y - rope_draw_len * 0.5);
        rope_obj.rotation = rope_ang + 90.0;
        rope_obj.visible = true;
        rope_obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (rope_beam, rope_draw_len), 0.0),
            image: rope_img,
            color: None,
        });
    }
}

/// Manage engine gravity. When hooked: gravity = 0 (rope handles it).
/// When free: gravity = GRAVITY * direction * zero-g scale.
/// During rocket launch (space_launch_active) and while in space: near-zero gravity.
pub fn sync_engine_gravity(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let target_gravity = if s.hooked {
        0.0
    } else if s.in_space_mode || s.space_launch_active {
        // Space / ascent: effectively no global gravity — planet wells do the work.
        GRAVITY * SPACE_GRAVITY_SCALE * s.gravity_dir
    } else {
        let g_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
        GRAVITY * g_scale * s.gravity_dir
    };
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = target_gravity;
    }
}

/// Clamp player momentum to MOMENTUM_CAP and write state back to engine.
/// The cap is bypassed while `space_launch_active` is true — the rocket pad
/// intentionally launches the player far beyond normal play speeds.
pub fn cap_momentum_and_write_back(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();

    if !s.space_launch_active {
        let cap = if s.in_space_mode { SPACE_MOMENTUM_CAP } else { MOMENTUM_CAP };
        let speed = (s.vx*s.vx + s.vy*s.vy).sqrt();
        if speed > cap {
            s.vx = s.vx / speed * cap;
            s.vy = s.vy / speed * cap;
        }
    }

    let (px, py, vx, vy) = (s.px, s.py, s.vx, s.vy);
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (px - PLAYER_R, py - PLAYER_R);
        obj.momentum = (vx, vy);
    }
}
