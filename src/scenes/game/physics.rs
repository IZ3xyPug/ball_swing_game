use quartz::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};

use image::AnimationDecoder;

use crate::constants::*;
use crate::shop::SHOP_ROPE_COLORS;
use crate::state::*;

static ROPE_FX_FRAMES: OnceLock<Vec<Arc<image::RgbaImage>>> = OnceLock::new();
static ROPE_FX_CACHE: OnceLock<Mutex<HashMap<(usize, u32, u32, u8, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
const ROPE_FX_SUPERSAMPLE: u32 = 1;
const ROPE_LEN_QUANTUM_PX: u32 = 20;
// How far the rope image extends past the hook center (away from player).
// Enough to hide cleanly behind the hook sprite but not protrude past it.
const ROPE_HOOK_PAD:   f32 = 62.0; // HOOK_R(38) + 24
const ROPE_HOOK_PAD_MAX_BONUS: f32 = 27.0;
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
fn rope_fx_image(
    frame_idx: usize,
    rope_len_px: u32,
    beam_px: u32,
    style_idx: u8,
    rope_rgb: (u8, u8, u8),
) -> Arc<image::RgbaImage> {
    let frames = rope_fx_frames();
    let idx = frame_idx % frames.len().max(1);
    let len_px = quantize_len_px(rope_len_px.max(1)).max(1);
    let thick_px = beam_px.max(1);
    let key = (idx, len_px, thick_px, style_idx, rope_rgb.0, rope_rgb.1, rope_rgb.2);

    let cache = ROPE_FX_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(img) = cache.lock().unwrap().get(&key).cloned() {
        return img;
    }

    let ss = ROPE_FX_SUPERSAMPLE.max(1);
    let target_w = thick_px.saturating_mul(ss);
    let target_h = len_px.saturating_mul(ss);
    let src = frames[idx].as_ref();
    let mut resized = image::imageops::resize(src, target_w, target_h, image::imageops::FilterType::Nearest);

    // Style 0 preserves the original animated energy rope unchanged.
    // Styles 1-4 are drawn fresh onto a blank canvas — completely new rope shapes,
    // not a recolor of the GIF.
    if style_idx != 0 {
        let w = target_w.max(1) as f32;
        let h = target_h.max(1) as f32;
        let phase = frame_idx as f32 * 0.27;
        let style = style_idx.min(4);
        let mut fresh = image::RgbaImage::new(target_w, target_h);
        for y in 0..target_h {
            for x in 0..target_w {
                let xf = x as f32 / w;
                let yf = y as f32 / h;

                let shade: f32 = match style {
                    // Braided rope: two crossing strands with a bright highlight core.
                    1 => {
                        let lane_a = 0.35 + 0.22 * (yf * 30.0 + phase).sin();
                        let lane_b = 0.65 + 0.22 * (yf * 30.0 + phase + PI).sin();
                        let da = (xf - lane_a).abs();
                        let db = (xf - lane_b).abs();
                        let near = da.min(db);
                        let strand = (1.0 - near / 0.18).clamp(0.0, 1.0);
                        // Highlight: bright center of each strand.
                        let hi_a = (1.0 - da / 0.06).clamp(0.0, 1.0);
                        let hi_b = (1.0 - db / 0.06).clamp(0.0, 1.0);
                        (strand * 0.7 + hi_a.max(hi_b) * 0.3).clamp(0.0, 1.0)
                    }
                    // Segmented cable: dark gaps between metallic link sections.
                    2 => {
                        let seg_len = 12u32;
                        let seg_phase = ((y / seg_len + (frame_idx / 2) as u32) % 2) as f32;
                        let core = (1.0 - (xf - 0.5).abs() / 0.40).clamp(0.0, 1.0);
                        // Gap at segment boundary.
                        let local_y = (y % seg_len) as f32 / seg_len as f32;
                        let gap = if local_y < 0.12 || local_y > 0.88 { 0.0 } else { 1.0 };
                        let brightness = if seg_phase > 0.5 { 0.90 } else { 0.55 };
                        core * gap * brightness
                    }
                    // Chain: oval links with dark holes in the centre.
                    3 => {
                        let link_h = 18u32;
                        let local_y = (y % link_h) as f32 / link_h as f32;
                        // Outer oval of the link.
                        let oy = (local_y - 0.5).abs() * 2.0; // 0 = mid, 1 = top/bot
                        let outer_r = 0.44 + 0.1 * (1.0 - oy);
                        let ox = (xf - 0.5).abs();
                        let on_link = (ox < outer_r) as u8 as f32;
                        // Cut out inner hole.
                        let inner_r = outer_r * 0.45;
                        let in_hole = (ox < inner_r && oy < 0.55) as u8 as f32;
                        // Pulsing highlight along the top of each link.
                        let hi = ((local_y * PI * 2.0 + phase).sin() * 0.5 + 0.5) * 0.35;
                        (on_link - in_hole).clamp(0.0, 1.0) * (0.65 + hi)
                    }
                    // Plasma ribbon: bright animated wave core.
                    _ => {
                        let wave = 0.5 + 0.5 * (yf * 44.0 + phase * 2.4 + xf * 8.0).sin();
                        let core = (1.0 - (xf - 0.5).abs() / 0.42).clamp(0.0, 1.0);
                        let glow_edge = (1.0 - (xf - 0.5).abs() / 0.50).clamp(0.0, 1.0);
                        0.15 * glow_edge + 0.85 * (0.55 * core + 0.45 * wave) * core
                    }
                };

                if shade > 0.02 {
                    let r = (rope_rgb.0 as f32 * shade).clamp(0.0, 255.0) as u8;
                    let g = (rope_rgb.1 as f32 * shade).clamp(0.0, 255.0) as u8;
                    let b = (rope_rgb.2 as f32 * shade).clamp(0.0, 255.0) as u8;
                    let a = (shade * 255.0).clamp(0.0, 255.0) as u8;
                    fresh.put_pixel(x, y, image::Rgba([r, g, b, a]));
                }
            }
        }
        resized = fresh;
    }
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
                for (style_idx, &(rr, rg, rb)) in SHOP_ROPE_COLORS.iter().enumerate() {
                    rope_fx_image(frame_idx, len, beam_px, style_idx as u8, (rr, rg, rb));
                }
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

    // All hooks (asteroid GIFs, space hooks, regular hooks) now drift, so
    // refresh the anchor from the object's current centre every tick.
    if !s.active_hook.is_empty() {
        let hook_id = s.active_hook.clone();
        drop(s);
        if let Some(hook_obj) = c.get_game_object(&hook_id) {
            let new_hx = hook_obj.position.0 + hook_obj.size.0 * 0.5;
            let new_hy = hook_obj.position.1 + hook_obj.size.1 * 0.5;
            s = st.lock().unwrap();
            s.hook_x = new_hx;
            s.hook_y = new_hy;
        } else {
            s = st.lock().unwrap();
        }
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

    // Extend farther behind hook as reach approaches max so long grapples
    // still visually touch the hook node without overextending short grabs.
    let reach_t = ((vis_dist - ROPE_LEN_MIN) / (ROPE_LEN_MAX - ROPE_LEN_MIN).max(1.0)).clamp(0.0, 1.0);
    let hook_pad = ROPE_HOOK_PAD + ROPE_HOOK_PAD_MAX_BONUS * reach_t;
    let start_x = hx - unit_x * hook_pad;
    let start_y = hy - unit_y * hook_pad;
    let end_x = vis_px + unit_x * ROPE_PLAYER_PAD;
    let end_y = vis_py + unit_y * ROPE_PLAYER_PAD;
    let seg_dx = end_x - start_x;
    let seg_dy = end_y - start_y;
    let rope_draw_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt().max(1.0);
    let rope_mid_x = (start_x + end_x) * 0.5;
    let rope_mid_y = (start_y + end_y) * 0.5;

    let rope_beam = ROPE_THICKNESS.max(2.0);
    let rope_beam_px = rope_beam.round().max(2.0) as u32;
    let rope_len_px = rope_draw_len.round().max(1.0) as u32;
    let frame_idx = ((rope_tick as usize) / 2) % rope_fx_frames().len().max(1);
    let (rope_sel_idx, rope_rgb) = {
        let idx = match c.get_var("player_rope_selected") {
            Some(Value::I32(v)) => v.max(0) as usize,
            _ => 0,
        }
        .min(SHOP_ROPE_COLORS.len() - 1);
        (idx as u8, SHOP_ROPE_COLORS[idx])
    };
    let rope_img = rope_fx_image(frame_idx, rope_len_px, rope_beam_px, rope_sel_idx, rope_rgb);

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
        // Skip speed cap on the frame immediately after a pad bounce so the full
        // bounce velocity is preserved. The flag is set by tick_pad_bounce and cleared here.
        let post_bounce = matches!(c.get_var("post_bounce"), Some(Value::Bool(true)));
        if post_bounce {
            c.set_var("post_bounce", false);
        } else {
            let cap = if s.in_space_mode { SPACE_MOMENTUM_CAP } else { MOMENTUM_CAP };
            let speed = (s.vx*s.vx + s.vy*s.vy).sqrt();
            if speed > cap {
                s.vx = s.vx / speed * cap;
                s.vy = s.vy / speed * cap;
            }
        }
    }

    let (px, py, vx, vy) = (s.px, s.py, s.vx, s.vy);
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (px - PLAYER_R, py - PLAYER_R);
        obj.momentum = (vx, vy);
    }
}
