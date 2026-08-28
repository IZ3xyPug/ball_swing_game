// ── scenes/game/gravity_cannon.rs ─────────────────────────────────────────────
//
// Gravity cannon obstacle behaviour:
//   Idle       → bob + display frame 8 at CANNON_DEFAULT_ROTATION
//   Capturing  → play pulse 8→7→6→7→8 (CANNON_CAPTURE_TICKS_PER_FRAME each)
//                player position frozen at cannon mouth
//   Charging   → CANNON_CHARGE_TICKS with player still frozen,
//                cannon rotates CW by CANNON_CHARGE_ROTATION_DEG
//   FiringDown → play frames 8→0 (CANNON_FIRE_TICKS_PER_FRAME each)
//                on frame 0: apply launch impulse + gravity damp + zero-g timer
//   FiringUp   → play frames 0→8 before rotation recovery
//   Recovering → CANNON_RECOVER_TICKS, rotate back to default rotation

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use super::helpers::center_warp_on_player;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Decode the gravitycannon.gif and return all raw RGBA frames (index 0..8).
fn cannon_frames_cached() -> &'static Vec<std::sync::Arc<image::RgbaImage>> {
    static FRAMES: std::sync::OnceLock<Vec<std::sync::Arc<image::RgbaImage>>> =
        std::sync::OnceLock::new();
    FRAMES.get_or_init(|| {
        use image::AnimationDecoder;
        let cursor = std::io::Cursor::new(ASSET_GRAVITYCANNON_GIF);
        let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) else {
            return Vec::new();
        };
        let Ok(raw_frames) = decoder.into_frames().collect_frames() else {
            return Vec::new();
        };

        let out_w = GRAVITYCANNON_W.round() as u32;
        let out_h = GRAVITYCANNON_H.round() as u32;

        raw_frames
            .into_iter()
            .map(|f| {
                let scaled = image::imageops::resize(
                    f.buffer(),
                    out_w,
                    out_h,
                    image::imageops::FilterType::Nearest,
                );
                std::sync::Arc::new(scaled)
            })
            .collect()
    })
}

#[inline]
fn set_cannon_frame(c: &mut Canvas, id: &str, frame_idx: usize) {
    let frames = cannon_frames_cached();
    let idx = frame_idx.min(frames.len().saturating_sub(1));
    if let Some(frame) = frames.get(idx) {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.animated_sprite = None;
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (GRAVITYCANNON_W, GRAVITYCANNON_H), 0.0), image: frame.clone(), color: None });
        }
    }
}

#[inline]
fn capture_pulse_frame(seq_idx: usize) -> usize {
    // 8→7→6→7→8 pulse while player is held in the cannon mouth.
    const CAPTURE_SEQ: [usize; 5] = [
        CANNON_DEFAULT_FRAME_INDEX,
        CANNON_DEFAULT_FRAME_INDEX - 1,
        CANNON_DEFAULT_FRAME_INDEX - 2,
        CANNON_DEFAULT_FRAME_INDEX - 1,
        CANNON_DEFAULT_FRAME_INDEX,
    ];
    CAPTURE_SEQ[seq_idx.min(CAPTURE_SEQ.len() - 1)]
}

/// Returns the world-space capture point (where the player is held while in
/// the barrel). For a cannon at `pos` rotated `rotation_deg`, the barrel mouth
/// is at the cannon centre offset along the barrel axis.
fn barrel_mouth_world(pos: (f32, f32), rotation_deg: f32) -> (f32, f32) {
    let cx = pos.0 + GRAVITYCANNON_W * 0.5;
    let cy = pos.1 + GRAVITYCANNON_H * 0.5;
    // Barrel extends in the local +X direction before rotation.
    // After a -90° rotation, +X maps to up (-Y), so "barrel mouth" is above centre.
    let rad = rotation_deg.to_radians();
    let barrel_len = GRAVITYCANNON_W * 0.55;
    (cx + barrel_len * rad.cos(), cy + barrel_len * rad.sin())
}

/// The resting barrel rotation for a cannon, accounting for gravity flip.
/// Normal gravity points the barrel up (-90°); flipped gravity mirrors it (+90°).
/// Mirror a barrel angle into UNFLIPPED screen space.
///
/// Flipped gravity draws the world upside down, so a barrel angle mirrors about
/// the horizontal axis: theta -> -theta. The flipped rest angle (+90, pointing
/// down) maps to the unflipped rest (-90, pointing up), and the flipped firing
/// angle maps to the unflipped one.
///
/// NOT a 180-degree rotation. A half-turn negates BOTH components of the launch
/// vector, so it reverses the direction of travel as well as the vertical — the
/// cannon fired the player back down the level. Flipped gravity mirrors the
/// world vertically; it does not send you the other way.
#[inline]
fn mirror_rotation(rotation_deg: f32, flipped: bool) -> f32 {
    if flipped { -rotation_deg } else { rotation_deg }
}

/// Which way the barrel sweeps while charging.
///
/// Screen Y points down, so a rising angle is clockwise. Unflipped that sweeps
/// the barrel from up toward forward-right, which is the shot. Mirrored, the
/// same rising angle sweeps from down toward BACKWARD-left — the cannon visibly
/// winding up the wrong way before firing the wrong way.
#[inline]
fn cannon_rot_dir(flipped: bool) -> f32 {
    if flipped { -1.0 } else { 1.0 }
}

fn cannon_default_rotation(flipped: bool) -> f32 {
    if flipped {
        CANNON_DEFAULT_ROTATION + 180.0
    } else {
        CANNON_DEFAULT_ROTATION
    }
}

/// Mirror all live gravity cannons for a gravity flip: mirror Y position and
/// bob base, and flip the barrel rotation so the cannon points the gravity way.
/// Called from `apply_flip_transform` when gravity flips.
pub fn flip_cannons(c: &mut Canvas, s: &mut State) {
    for phase in s.cannon_phases.iter_mut() {
        phase.flipped = !phase.flipped;
        phase.base_y = VH - phase.base_y - GRAVITYCANNON_H;
        phase.rotation = cannon_default_rotation(phase.flipped);
        if let Some(obj) = c.get_game_object_mut(&phase.id) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
            obj.rotation = phase.rotation;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spawning
// ─────────────────────────────────────────────────────────────────────────────

pub fn spawn_cannons(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.in_space_mode { return; }

    while s.cannon_rightmost < s.px + GEN_AHEAD && !s.cannon_free.is_empty() {
        let gap = lcg_range(&mut s.seed, CANNON_GAP_MIN, CANNON_GAP_MAX);
        let x = s.cannon_rightmost + gap;
        // Place in the middle band so the cannon floats visibly.
        let base_y = lcg_range(&mut s.seed, VH * 0.25, VH * 0.65);
        let bob_phase = lcg_range(&mut s.seed, 0.0, std::f32::consts::TAU);
        let Some(id) = s.cannon_free.pop() else { break; };
        let flipped = s.gravity_dir < 0.0;
        s.cannon_live.push(id.clone());
        s.cannon_rightmost = x;
        s.cannon_phases.push(CannonPhase {
            id:        id.clone(),
            state:     CannonState::Idle,
            base_y,
            bob_phase,
            rotation:  cannon_default_rotation(flipped),
            flipped,
        });
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let bob_y = base_y; // starts at base; bob applied each tick
            obj.position = (x - GRAVITYCANNON_W * 0.5, bob_y - GRAVITYCANNON_H * 0.5);
            obj.momentum = (0.0, 0.0);
            obj.rotation = cannon_default_rotation(flipped);
            obj.layer = 30;
            obj.visible = true;
            set_cannon_frame(c, &id, CANNON_DEFAULT_FRAME_INDEX);
        }

        s = st.lock().unwrap();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Culling
// ─────────────────────────────────────────────────────────────────────────────

pub fn cull_cannons(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.cannon_live.is_empty() { return; }
    if s.cannon_captured { return; } // never cull while player is inside

    let cutoff = s.px - VW * 3.0;
    let to_remove: Vec<String> = s.cannon_live.iter()
        .filter(|id| {
            c.get_game_object(id)
                .map(|o| o.position.0 + GRAVITYCANNON_W < cutoff)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    for id in &to_remove {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
            obj.layer = 30;
        }
    }

    if !to_remove.is_empty() {
        use std::collections::HashSet;
        let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
        s.cannon_live.retain(|n| !rm.contains(n.as_str()));
        s.cannon_phases.retain(|p| !rm.contains(p.id.as_str()));
        for id in to_remove { s.cannon_free.push(id); }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-tick behaviour
// ─────────────────────────────────────────────────────────────────────────────

pub fn tick_cannons(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let frames_available = cannon_frames_cached().len();
    if frames_available == 0 { return; }

    // ── Snapshot all needed state in ONE lock — avoids every double-lock deadlock ─
    let (
        player_px,
        player_py,
        mut any_captured,
        ticks,
        damp_timer,
        zero_g_timer,
        gravity_dir,
        player_hooked,
        coin_count,
        ft_active,
        ft_prompt,
        phases,
    ) = {
        let s = st.lock().unwrap();
        (
            s.px, s.py, s.cannon_captured,
            s.ticks, s.cannon_damp_timer, s.zero_g_timer, s.gravity_dir,
            s.hooked,
            s.coin_count,
            s.cannon_ft_active,
            s.cannon_ft_prompt,
            s.cannon_phases.clone(),
        )
    };

    let mut updated_phases: Vec<CannonPhase> = Vec::with_capacity(phases.len());
    let mut launch_impulse: Option<(f32, f32)> = None;
    let mut release_player = false;
    let mut begin_damp = false;
    let mut do_fast_travel = false;
    let mut pull_impulse = (0.0_f32, 0.0_f32);

    for mut phase in phases {
        // ── Bob (all states) ──────────────────────────────────────────────────
        let bob_y = phase.base_y
            + (ticks as f32 * CANNON_BOB_SPEED + phase.bob_phase).sin() * CANNON_BOB_AMP;
        if let Some(obj) = c.get_game_object_mut(&phase.id) {
            obj.position.1 = bob_y - GRAVITYCANNON_H * 0.5;
            obj.rotation = phase.rotation;
        }

        match phase.state.clone() {
            CannonState::Idle => {
                let obj_pos = c.get_game_object(&phase.id).map(|o| o.position);
                if let Some(pos) = obj_pos {
                    let cx = pos.0 + GRAVITYCANNON_W * 0.5;
                    let cy = pos.1 + GRAVITYCANNON_H * 0.5;
                    let dx = player_px - cx;
                    let dy = player_py - cy;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 <= CANNON_TRIGGER_RADIUS * CANNON_TRIGGER_RADIUS
                        && !any_captured
                    {
                        // Mark captured locally so subsequent cannons can't also trigger.
                        any_captured = true;
                        // Single lock: mutate all state fields, extract hooked status.
                        let was_hooked = {
                            let mut s = st.lock().unwrap();
                            s.cannon_captured = true;
                            s.cannon_capture_id = phase.id.clone();
                            s.vx = 0.0;
                            s.vy = 0.0;
                            // Show the fast-travel prompt if the player can afford it;
                            // they must press F to accept (set s.cannon_ft_active).
                            s.cannon_ft_prompt = coin_count >= CANNON_FAST_TRAVEL_COST;
                            s.cannon_ft_active = false;
                            let h = s.hooked;
                            if h {
                                s.hooked = false;
                                s.active_hook = String::new();
                            }
                            h
                        };
                        if was_hooked {
                            c.run(Action::Hide { target: Target::name("rope") });
                        }
                        if let Some(obj) = c.get_game_object_mut(&phase.id) {
                            obj.layer = LAYER_CANNON_ACTIVE;
                        }
                        phase.state = CannonState::Capturing {
                            seq_idx:     0,
                            frame_timer: CANNON_CAPTURE_TICKS_PER_FRAME,
                        };
                    } else if !any_captured && !player_hooked
                        && dist2 <= CANNON_PULL_RADIUS * CANNON_PULL_RADIUS
                    {
                        // Gentle attractor to guide the player into capture range.
                        let mouth = barrel_mouth_world(pos, phase.rotation);
                        let to_mx = mouth.0 - player_px;
                        let to_my = mouth.1 - player_py;
                        let to_m_len = (to_mx * to_mx + to_my * to_my).sqrt();
                        if to_m_len > 0.001 {
                            let dist = dist2.sqrt();
                            let t = (1.0 - dist / CANNON_PULL_RADIUS).clamp(0.0, 1.0);
                            let accel = CANNON_PULL_ACCEL * t * t;
                            pull_impulse.0 += (to_mx / to_m_len) * accel;
                            pull_impulse.1 += (to_my / to_m_len) * accel;
                        }
                    }
                }
            }

            CannonState::Capturing { seq_idx, frame_timer } => {
                let obj_pos = c.get_game_object(&phase.id).map(|o| o.position);
                if let Some(pos) = obj_pos {
                    let mouth = barrel_mouth_world(pos, phase.rotation);
                    let mut s = st.lock().unwrap();
                    s.px = mouth.0;
                    s.py = mouth.1;
                    s.vx = 0.0;
                    s.vy = 0.0;
                }
                // Receiving cannon: while the fast-travel warp flash plays, hold
                // ~45° to the LEFT of rest (barrel up-left) so the player is
                // clearly "caught" on the upper left; once it fades, settle to
                // the fire angle and only then advance the capture pulse (so the
                // final charge/fire is from the correct rest angle, rotating
                // forward/right to launch).
                let rest = cannon_default_rotation(phase.flipped);
                let warp_active = matches!(c.get_var("warp_flash_ticks"), Some(Value::I32(v)) if v > 0);
                let revealing = matches!(c.get_var("cannon_ft_reveal_ticks"), Some(Value::I32(v)) if v > 0);
                if warp_active || revealing {
                    phase.rotation = rest - 45.0 * cannon_rot_dir(phase.flipped);
                    if revealing {
                        let mut v = match c.get_var("cannon_ft_reveal_ticks") { Some(Value::I32(n)) => n, _ => 0 };
                        v = v.saturating_sub(1);
                        c.set_var("cannon_ft_reveal_ticks", v);
                    }
                    updated_phases.push(phase);
                    continue;
                }
                phase.rotation += (rest - phase.rotation) * 0.30;
                let settled = (phase.rotation - rest).abs() < 3.0;
                let mut new_seq = seq_idx;
                let mut new_timer = frame_timer;
                if !settled {
                    // Keep holding the player until the cannon has rotated to rest.
                } else if new_timer == 0 {
                    if new_seq + 1 < 5 {
                        new_seq += 1;
                        new_timer = CANNON_CAPTURE_TICKS_PER_FRAME;
                        set_cannon_frame(c, &phase.id, capture_pulse_frame(new_seq));
                    } else {
                        // Pulse complete. If the player can afford fast-travel,
                        // hold and wait for the press-F choice instead of
                        // charging straight into a launch.
                        if ft_prompt {
                            phase.state = CannonState::WaitingChoice { ticks: CANNON_CHOICE_WAIT_TICKS };
                        } else {
                            phase.state = CannonState::Charging { ticks: CANNON_CHARGE_TICKS };
                        }
                        updated_phases.push(phase);
                        continue;
                    }
                } else {
                    new_timer -= 1;
                }
                phase.state = CannonState::Capturing { seq_idx: new_seq, frame_timer: new_timer };
            }

            CannonState::WaitingChoice { mut ticks } => {
                let obj_pos = c.get_game_object(&phase.id).map(|o| o.position);
                if let Some(pos) = obj_pos {
                    let mouth = barrel_mouth_world(pos, phase.rotation);
                    let mut s = st.lock().unwrap();
                    s.px = mouth.0;
                    s.py = mouth.1;
                    s.vx = 0.0;
                    s.vy = 0.0;
                }
                if ft_active {
                    // Player pressed F: accept fast-travel — run the full
                    // launch (rotate/charge then fire) before the warp so the
                    // cannon clearly launches the player, THEN fast-travel.
                    phase.state = CannonState::Charging { ticks: CANNON_CHARGE_TICKS };
                } else if ticks == 0 {
                    // Timed out: clear the prompt and launch normally.
                    let mut s = st.lock().unwrap();
                    s.cannon_ft_prompt = false;
                    s.cannon_ft_active = false;
                    phase.state = CannonState::Charging { ticks: CANNON_CHARGE_TICKS };
                } else {
                    phase.state = CannonState::WaitingChoice { ticks: ticks - 1 };
                }
            }

            CannonState::Charging { ticks } => {
                let obj_pos = c.get_game_object(&phase.id).map(|o| o.position);
                if let Some(pos) = obj_pos {
                    let mouth = barrel_mouth_world(pos, phase.rotation);
                    let mut s = st.lock().unwrap();
                    s.px = mouth.0;
                    s.py = mouth.1;
                    s.vx = 0.0;
                    s.vy = 0.0;
                }
                let rot_step = (CANNON_CHARGE_ROTATION_DEG / CANNON_CHARGE_TICKS as f32)
                    * cannon_rot_dir(phase.flipped);
                phase.rotation += rot_step;
                if ticks == 0 {
                    set_cannon_frame(c, &phase.id, CANNON_DEFAULT_FRAME_INDEX);
                    phase.state = CannonState::FiringDown {
                        frame_idx:   CANNON_DEFAULT_FRAME_INDEX,
                        frame_timer: CANNON_FIRE_TICKS_PER_FRAME,
                    };
                } else {
                    phase.state = CannonState::Charging { ticks: ticks - 1 };
                }
            }

            CannonState::FiringDown { frame_idx, frame_timer } => {
                let obj_pos = c.get_game_object(&phase.id).map(|o| o.position);
                if let Some(pos) = obj_pos {
                    let mouth = barrel_mouth_world(pos, phase.rotation);
                    let mut s = st.lock().unwrap();
                    s.px = mouth.0;
                    s.py = mouth.1;
                    s.vx = 0.0;
                    s.vy = 0.0;
                }
                let mut new_frame = frame_idx;
                let mut new_timer = frame_timer;
                if new_timer == 0 {
                    if new_frame > 0 {
                        new_frame -= 1;
                        new_timer = CANNON_FIRE_TICKS_PER_FRAME;
                        set_cannon_frame(c, &phase.id, new_frame);
                    } else {
                        // Launch direction must be derived from the UNFLIPPED
                        // rotation, then mirrored vertically.
                        //
                        // `cannon_default_rotation` adds 180 degrees when
                        // gravity is flipped, which is right for the barrel
                        // SPRITE — the world is drawn upside down, so the art
                        // has to turn over. But rotating the launch vector by
                        // 180 negates BOTH components, so the horizontal
                        // component flipped too and the cannon fired the player
                        // backwards down the level. Flipped gravity mirrors the
                        // world vertically; it does not reverse the direction of
                        // travel.
                        // Use the orientation the BARREL was animated in, not
                        // the world's orientation right now. Those disagree if
                        // gravity flips back mid-charge, and then the mirror is
                        // applied to a rotation that was swept the other way —
                        // firing the player backwards on exactly the launches
                        // that straddle the flip.
                        let flipped = phase.flipped;
                        let base_rot = mirror_rotation(phase.rotation, flipped);
                        let rot_rad = base_rot.to_radians();
                        let vx = CANNON_LAUNCH_VX * rot_rad.cos() - CANNON_LAUNCH_VY * rot_rad.sin();
                        let vy_unflipped =
                            CANNON_LAUNCH_VX * rot_rad.sin() + CANNON_LAUNCH_VY * rot_rad.cos();
                        let vy = if flipped { -vy_unflipped } else { vy_unflipped };
                        if ft_active {
                            // Hyper-transit: teleport far ahead instead of the short launch.
                            do_fast_travel = true;
                        } else {
                            launch_impulse = Some((vx, vy));
                            release_player = true;
                            begin_damp = true;
                        }
                        if let Some(obj) = c.get_game_object_mut(&phase.id) {
                            obj.layer = 30;
                        }
                        phase.state = CannonState::FiringUp {
                            frame_idx: 0,
                            frame_timer: CANNON_FIRE_TICKS_PER_FRAME,
                        };
                        updated_phases.push(phase);
                        continue;
                    }
                } else {
                    new_timer -= 1;
                }
                phase.state = CannonState::FiringDown { frame_idx: new_frame, frame_timer: new_timer };
            }

            CannonState::FiringUp { frame_idx, frame_timer } => {
                let mut new_frame = frame_idx;
                let mut new_timer = frame_timer;
                if new_timer == 0 {
                    if new_frame + 1 <= CANNON_DEFAULT_FRAME_INDEX {
                        new_frame += 1;
                        new_timer = CANNON_FIRE_TICKS_PER_FRAME;
                        set_cannon_frame(c, &phase.id, new_frame);
                    } else {
                        phase.state = CannonState::Recovering { ticks: CANNON_RECOVER_TICKS };
                        updated_phases.push(phase);
                        continue;
                    }
                } else {
                    new_timer -= 1;
                }
                phase.state = CannonState::FiringUp { frame_idx: new_frame, frame_timer: new_timer };
            }

            CannonState::Recovering { ticks } => {
                let target = cannon_default_rotation(phase.flipped);
                let diff = target - phase.rotation;
                let step = diff / ticks.max(1) as f32;
                phase.rotation += step;
                if ticks == 0 {
                    phase.rotation = target;
                    set_cannon_frame(c, &phase.id, CANNON_DEFAULT_FRAME_INDEX);
                    phase.state = CannonState::Idle;
                } else {
                    phase.state = CannonState::Recovering { ticks: ticks - 1 };
                }
            }
        }

        updated_phases.push(phase);
    }

    // ── Write all state changes back in one lock ──────────────────────────────
    {
        let mut s = st.lock().unwrap();
        if let Some((vx, vy)) = launch_impulse { s.vx = vx; s.vy = vy; }
        if release_player { s.cannon_captured = false; s.cannon_capture_id = String::new(); }
        if begin_damp { s.cannon_damp_timer = CANNON_GRAVITY_DAMP_TICKS; }
        if !s.cannon_captured && !s.hooked {
            s.vx += pull_impulse.0;
            s.vy += pull_impulse.1;
            let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
            if speed > CANNON_PULL_SPEED_CAP {
                let k = CANNON_PULL_SPEED_CAP / speed;
                s.vx *= k;
                s.vy *= k;
            }
        }
        if s.cannon_damp_timer > 0 { s.cannon_damp_timer -= 1; }
        if s.cannon_fast_travel_grace > 0 { s.cannon_fast_travel_grace -= 1; }
        s.cannon_phases = updated_phases;
    }

    // ── Fast travel: teleport far ahead, grant a random buff ───────────────
    if do_fast_travel {
        fast_travel_player(c, st);
    }

    // ── Apply gravity override to player object (uses snapshot values, no lock) ─
    if damp_timer > 0 {
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * GRAVITY_DAMP_SCALE * gravity_dir;
        }
    } else if !any_captured {
        let grav = if zero_g_timer > 0 {
            GRAVITY * ZERO_G_GRAVITY_SCALE * gravity_dir
        } else {
            GRAVITY * gravity_dir
        };
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = grav;
        }
    }
}

/// Draw a thin translucent line for the speed-line warp effect.
fn draw_line(img: &mut image::RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 4]) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (dx.abs().max(dy.abs())).ceil().max(1.0) as u32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (x0 + dx * t).round() as i32;
        let y = (y0 + dy * t).round() as i32;
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            let x = x as u32;
            let y = y as u32;
            img.put_pixel(x, y, image::Rgba(c));
            if x + 1 < img.width() {
                img.put_pixel(x + 1, y, image::Rgba(c));
            }
        }
    }
}

/// Radial "warp speed" streaks used as the fast-travel arrival overlay.
fn speed_lines_img(w: u32, h: u32, alpha: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let mut seed = 0x1234_5678u64;
    let min_dim = w.min(h) as f32;
    let max_dim = w.max(h) as f32;
    let count = 110;
    for _ in 0..count {
        let ang = lcg(&mut seed) * std::f32::consts::TAU;
        let inner = (0.04 + 0.10 * lcg(&mut seed)) * min_dim;
        let outer = (0.45 + 0.55 * lcg(&mut seed)) * max_dim;
        let ca = ang.cos();
        let sa = ang.sin();
        let x0 = cx + ca * inner;
        let y0 = cy + sa * inner;
        let x1 = cx + ca * outer;
        let y1 = cy + sa * outer;
        let streak = [
            (170 + (lcg(&mut seed) * 60.0) as u8),
            235,
            255,
            alpha,
        ];
        draw_line(&mut img, x0, y0, x1, y1, streak);
    }
    img
}

/// Hyper-transit: consume the coin cost, teleport the player far ahead, grant a
/// random run-long buff, rewind the spawn frontiers so the destination world is
/// generated, and end at an existing procedurally-generated gravity cannon
/// (spawning one only if none is nearby), where the player is held in stasis and
/// launched normally. Also plays a high-speed warp overlay.
/// Hyper-transit: consume the coin cost, grant a random run-long buff and rewind
/// the spawn frontiers so the destination world is generated. The actual
/// teleport and receiving-cannon are deferred: first an *outgoing* speed-line
/// phase plays centred on the player at the launching cannon, then a white
/// screen flash hides the cut, then a *reverse* speed-line phase plays centred
/// on the player at the receiving cannon (see `tick_cannon_warp`).
fn fast_travel_player(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (dest_x, dest_y, lx, ly) = {
        let mut s = st.lock().unwrap();
        let dest_x = s.px + CANNON_FAST_TRAVEL_DISTANCE;
        let dest_y = (VH * 0.5).clamp(60.0, VH - 60.0);
        s.coin_count = s.coin_count.saturating_sub(CANNON_FAST_TRAVEL_COST);

        // Random persistent buff (lasts until the run ends).
        let roll = lcg(&mut s.seed);
        if roll < 0.34 {
            s.max_hearts += 1;
            s.hearts += 1;
        } else if roll < 0.67 {
            s.oxygen_drain_scale = UPGRADE_BREATH_DRAIN_SCALE;
        } else {
            s.upgrade_momentum_bonus = true;
        }

        s.cannon_ft_active = false;
        s.cannon_ft_prompt = false;
        s.cannon_fast_travel_grace = CANNON_FAST_TRAVEL_GRACE;
        s.hooked = false;
        s.active_hook = String::new();
        s.vx = 0.0;
        s.vy = 0.0;
        s.in_space_mode = false;
        s.space_launch_active = false;
        // Normalize gravity at the destination: a space_rift gravity flip that
        // was active when fast-travel fired must not stick after arrival.
        s.gravity_dir = 1.0;
        s.flip_timer = 0;

        // Rewind spawn frontiers AND clear the stale pre-generated pending
        // queue so hooks/tether nodes/obstacles regenerate around the
        // destination instead of staying far ahead.
        let backfill_x = dest_x - GEN_AHEAD * 0.3;
        s.rightmost_x = s.rightmost_x.min(backfill_x);
        s.pad_rightmost = s.pad_rightmost.min(backfill_x);
        s.spinner_rightmost = s.spinner_rightmost.min(backfill_x);
        s.coin_rightmost = s.coin_rightmost.min(backfill_x);
        s.flip_rightmost = s.flip_rightmost.min(backfill_x);
        s.score_x2_rightmost = s.score_x2_rightmost.min(backfill_x);
        s.zero_g_rightmost = s.zero_g_rightmost.min(backfill_x);
        s.gate_rightmost = s.gate_rightmost.min(backfill_x);
        s.gwell_rightmost = s.gwell_rightmost.min(backfill_x);
        s.turret_rightmost = s.turret_rightmost.min(backfill_x);
        s.rocket_pad_rightmost = s.rocket_pad_rightmost.min(backfill_x);
        s.pending.clear();
        let mut seed = s.seed;
        let mut gen_head_x = s.gen_head_x.min(backfill_x);
        let mut gen_head_y = s.gen_head_y;
        let batch = gen_hook_batch(&mut seed, backfill_x, &mut gen_head_x, &mut gen_head_y, s.distance);
        s.seed = seed;
        s.gen_head_x = gen_head_x;
        s.gen_head_y = gen_head_y;
        s.pending.extend(batch);
        // Cannons: land on an existing procedural cannon ahead if one is close,
        // otherwise plant one here. Keep the spawner from double-planting.
        s.cannon_rightmost = dest_x;
        // The state holds the player at the launching cannon's mouth (set by the
        // firing state); use that as the outgoing speed-line centre.
        (dest_x, dest_y, s.px, s.py)
    };

    // Remember the destination + the launch centre so the deferred teleport and
    // the incoming warp can use them after the outgoing speed-line phase.
    c.set_var("cannon_warp_dx", Value::F32(dest_x));
    c.set_var("cannon_warp_dy", Value::F32(dest_y));
    c.set_var("cannon_warp_cx", Value::F32(lx));
    c.set_var("cannon_warp_cy", Value::F32(ly));

    // Outgoing warp speed lines, centred on the player at the launching cannon.
    if let Some(obj) = c.get_game_object_mut("warp_flash") {
        obj.size = (VW, VH);
        obj.position = (lx - VW * 0.5, ly - VH * 0.5);
        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: speed_lines_img(VW as u32, VH as u32, 235).into(), color: None });
        obj.visible = true;
    }
    c.set_var("cannon_warp_phase", Value::I32(1));
    c.set_var("warp_flash_ticks", CANNON_WARP_OUT_TICKS);
    c.set_var("cannon_ft_reveal", false);
    c.set_var("cannon_ft_reveal_ticks", 0i32);
}

/// Perform the actual fast-travel teleport: pick/plant the receiving cannon,
/// capture the player at it, and snap the camera. Called at the outgoing →
/// incoming warp transition, hidden behind the white flash.
fn perform_cannon_warp_teleport(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (dest_x, dest_y, receiving_id) = {
        let mut s = st.lock().unwrap();
        let dest_x = match c.get_var("cannon_warp_dx") {
            Some(Value::F32(v)) => v,
            _ => s.px + CANNON_FAST_TRAVEL_DISTANCE,
        };
        let dest_y = match c.get_var("cannon_warp_dy") {
            Some(Value::F32(v)) => v,
            _ => VH * 0.5,
        };

        // Prefer an existing procedurally-generated cannon ahead of the
        // destination so we don't end up with two cannons overlapping at exit.
        let mut receiving_id: Option<String> = None;
        let mut chosen_dest_x = dest_x;
        let mut best: Option<(f32, String)> = None;
        for phase in &s.cannon_phases {
            if let Some(obj) = c.get_game_object(&phase.id) {
                let cx = obj.position.0 + GRAVITYCANNON_W * 0.5;
                if cx >= dest_x - 800.0 {
                    let d = (cx - dest_x).abs();
                    if best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                        best = Some((cx, phase.id.clone()));
                    }
                }
            }
        }
        if let Some((cx, id)) = best {
            // Land on this cannon: capture the player at it and launch normally.
            receiving_id = Some(id.clone());
            chosen_dest_x = cx;
            let mut windup_rot = 0.0;
            for phase in s.cannon_phases.iter_mut() {
                if phase.id == id {
                    phase.state = CannonState::Capturing { seq_idx: 0, frame_timer: CANNON_CAPTURE_TICKS_PER_FRAME };
                    // Arrive ~45° LEFT of rest so the cannon clearly catches the
                    // player on the upper-left, then rotates forward/right to fire.
                    phase.rotation = cannon_default_rotation(phase.flipped)
                        - 45.0 * cannon_rot_dir(phase.flipped);
                    windup_rot = phase.rotation;
                }
            }
            if let Some(obj) = c.get_game_object_mut(&id) {
                obj.layer = LAYER_CANNON_ACTIVE;
                obj.visible = true;
                obj.rotation = windup_rot;
            }
            s.cannon_captured = true;
            s.cannon_capture_id = id.clone();
        } else if let Some(id) = s.cannon_free.pop() {
            let base_y = dest_y + 60.0; // place the barrel mouth just above centre
            let flipped = s.gravity_dir < 0.0;
            let windup = cannon_default_rotation(flipped) - 45.0;
            s.cannon_live.push(id.clone());
            s.cannon_phases.push(CannonPhase {
                id:        id.clone(),
                state:     CannonState::Capturing { seq_idx: 0, frame_timer: CANNON_CAPTURE_TICKS_PER_FRAME },
                base_y,
                bob_phase: 0.0,
                rotation:  windup,
                flipped,
            });
            s.cannon_captured = true;
            s.cannon_capture_id = id.clone();
            receiving_id = Some(id.clone());
            set_cannon_frame(c, &id, CANNON_DEFAULT_FRAME_INDEX);
            if let Some(obj) = c.get_game_object_mut(&id) {
                obj.position = (dest_x - GRAVITYCANNON_W * 0.5, base_y - GRAVITYCANNON_H * 0.5);
                obj.momentum = (0.0, 0.0);
                obj.rotation = windup;
                obj.layer = LAYER_CANNON_ACTIVE;
                obj.visible = true;
            }
        } else {
            s.cannon_captured = false;
            s.cannon_capture_id = String::new();
        }

        s.px = chosen_dest_x;
        s.py = dest_y;
        (chosen_dest_x, dest_y, receiving_id)
    };

    if let Some(id) = &receiving_id {
        set_cannon_frame(c, id, CANNON_DEFAULT_FRAME_INDEX);
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = true;
            obj.layer = LAYER_CANNON_ACTIVE;
        }
    }

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (dest_x - PLAYER_R, dest_y - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.gravity = 0.0;
        obj.visible = true;
    }
    c.run(Action::Hide { target: Target::name("rope") });
    c.set_var("rope_visible_at_pause", false);

    // Snap the camera to the receiver so the incoming speed lines are framed.
    if let Some(cam) = c.camera_mut() {
        cam.position = (dest_x - VW * 0.5, dest_y - VH * 0.5);
        cam.snap_zoom(1.0);
    }
}

/// Drive the two-phase fast-travel warp (outgoing speed lines → white flash —
/// teleport → reverse speed lines). The overlay stays centred on the player so
/// the speed-line origin tracks the ball throughout.
pub fn tick_cannon_warp(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let phase = match c.get_var("cannon_warp_phase") {
        Some(Value::I32(v)) => v,
        _ => 0,
    };
    if phase == 0 {
        return;
    }
    let ticks = match c.get_var("warp_flash_ticks") {
        Some(Value::I32(v)) => v,
        _ => 0,
    };

    if phase == 1 {
        // Outgoing: hold the player still at the launching cannon mouth so they
        // don't drift/fall while the speed lines play, then centre the overlay.
        let (cx, cy) = {
            let cx = match c.get_var("cannon_warp_cx") { Some(Value::F32(v)) => v, _ => 0.0 };
            let cy = match c.get_var("cannon_warp_cy") { Some(Value::F32(v)) => v, _ => 0.0 };
            (cx, cy)
        };
        {
            let mut s = st.lock().unwrap();
            s.px = cx;
            s.py = cy;
            s.vx = 0.0;
            s.vy = 0.0;
        }
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position = (cx - PLAYER_R, cy - PLAYER_R);
            obj.momentum = (0.0, 0.0);
            obj.gravity = 0.0;
        }
        center_warp_on_player(c);
        if ticks <= 0 {
            // Transition: the flash hides the cut, then teleport + incoming.
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(255, 255, 255, 215), 0.5, FlashMode::FadeOut, FlashEase::Smooth, 1.0, 0.10);
                cam.shake(18.0, 0.35);
            }
            perform_cannon_warp_teleport(c, st);
            // Centre the incoming reverse speed lines on the player at the
            // destination before the next tick re-centres them.
            let (dx, dy) = {
                let dx = match c.get_var("cannon_warp_dx") { Some(Value::F32(v)) => v, _ => 0.0 };
                let dy = match c.get_var("cannon_warp_dy") { Some(Value::F32(v)) => v, _ => 0.0 };
                (dx, dy)
            };
            if let Some(obj) = c.get_game_object_mut("warp_flash") {
                obj.position = (dx - VW * 0.5, dy - VH * 0.5);
            }
            c.set_var("cannon_warp_phase", Value::I32(2));
            c.set_var("warp_flash_ticks", CANNON_WARP_IN_TICKS);
        } else {
            c.set_var("warp_flash_ticks", ticks - 1);
        }
    } else if phase == 2 {
        // Incoming: reverse speed lines centred on the player at the receiver.
        center_warp_on_player(c);
        if ticks <= 0 {
            // Warp done; hand control back so the receiving cannon settles/fires.
            c.set_var("cannon_warp_phase", Value::I32(0));
            if let Some(obj) = c.get_game_object_mut("warp_flash") {
                obj.visible = false;
            }
        } else {
            c.set_var("warp_flash_ticks", ticks - 1);
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Debug helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Force-spawn one gravity cannon just ahead of the player for testing.
/// Bound to key G. No-op if the free pool is empty.
pub fn debug_spawn_cannon(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (x, id, base_y, flipped) = {
        let mut s = st.lock().unwrap();
        let Some(id) = s.cannon_free.pop() else { return; };
        let x   = s.px + VW * 0.25;
        let y   = VH  * 0.50;
        let flipped = s.gravity_dir < 0.0;
        s.cannon_live.push(id.clone());
        s.cannon_phases.push(CannonPhase {
            id:        id.clone(),
            state:     CannonState::Idle,
            base_y:    y,
            bob_phase: 0.0,
            rotation:  cannon_default_rotation(flipped),
            flipped,
        });
        (x, id, y, flipped)
    };
    set_cannon_frame(c, &id, CANNON_DEFAULT_FRAME_INDEX);
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.position = (x - GRAVITYCANNON_W * 0.5, base_y - GRAVITYCANNON_H * 0.5);
        obj.momentum = (0.0, 0.0);
        obj.rotation = cannon_default_rotation(flipped);
        obj.layer    = 30;
        obj.visible  = true;
    }
}
