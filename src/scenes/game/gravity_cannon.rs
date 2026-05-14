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
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (GRAVITYCANNON_W, GRAVITYCANNON_H), 0.0),
                image: frame.clone(),
                color: None,
            });
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
        s.cannon_live.push(id.clone());
        s.cannon_rightmost = x;
        s.cannon_phases.push(CannonPhase {
            id:        id.clone(),
            state:     CannonState::Idle,
            base_y,
            bob_phase,
            rotation:  CANNON_DEFAULT_ROTATION,
        });
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let bob_y = base_y; // starts at base; bob applied each tick
            obj.position = (x - GRAVITYCANNON_W * 0.5, bob_y - GRAVITYCANNON_H * 0.5);
            obj.momentum = (0.0, 0.0);
            obj.rotation = CANNON_DEFAULT_ROTATION;
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
        phases,
    ) = {
        let s = st.lock().unwrap();
        (
            s.px, s.py, s.cannon_captured,
            s.ticks, s.cannon_damp_timer, s.zero_g_timer, s.gravity_dir,
            s.hooked,
            s.cannon_phases.clone(),
        )
    };

    let mut updated_phases: Vec<CannonPhase> = Vec::with_capacity(phases.len());
    let mut launch_impulse: Option<(f32, f32)> = None;
    let mut release_player = false;
    let mut begin_damp = false;
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
                let mut new_seq = seq_idx;
                let mut new_timer = frame_timer;
                if new_timer == 0 {
                    if new_seq + 1 < 5 {
                        new_seq += 1;
                        new_timer = CANNON_CAPTURE_TICKS_PER_FRAME;
                        set_cannon_frame(c, &phase.id, capture_pulse_frame(new_seq));
                    } else {
                        phase.state = CannonState::Charging { ticks: CANNON_CHARGE_TICKS };
                        updated_phases.push(phase);
                        continue;
                    }
                } else {
                    new_timer -= 1;
                }
                phase.state = CannonState::Capturing { seq_idx: new_seq, frame_timer: new_timer };
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
                let rot_step = CANNON_CHARGE_ROTATION_DEG / CANNON_CHARGE_TICKS as f32;
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
                        let rot_rad = phase.rotation.to_radians();
                        let vx = CANNON_LAUNCH_VX * rot_rad.cos() - CANNON_LAUNCH_VY * rot_rad.sin();
                        let vy = CANNON_LAUNCH_VX * rot_rad.sin() + CANNON_LAUNCH_VY * rot_rad.cos();
                        launch_impulse = Some((vx, vy));
                        release_player = true;
                        begin_damp = true;
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
                let target = CANNON_DEFAULT_ROTATION;
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
        s.cannon_phases = updated_phases;
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

// ─────────────────────────────────────────────────────────────────────────────
// Debug helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Force-spawn one gravity cannon just ahead of the player for testing.
/// Bound to key G. No-op if the free pool is empty.
pub fn debug_spawn_cannon(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (x, id, base_y) = {
        let mut s = st.lock().unwrap();
        let Some(id) = s.cannon_free.pop() else { return; };
        let x   = s.px + VW * 0.25;
        let y   = VH  * 0.50;
        s.cannon_live.push(id.clone());
        s.cannon_phases.push(CannonPhase {
            id:        id.clone(),
            state:     CannonState::Idle,
            base_y:    y,
            bob_phase: 0.0,
            rotation:  CANNON_DEFAULT_ROTATION,
        });
        (x, id, y)
    };
    set_cannon_frame(c, &id, CANNON_DEFAULT_FRAME_INDEX);
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.position = (x - GRAVITYCANNON_W * 0.5, base_y - GRAVITYCANNON_H * 0.5);
        obj.momentum = (0.0, 0.0);
        obj.rotation = CANNON_DEFAULT_ROTATION;
        obj.layer    = 30;
        obj.visible  = true;
    }
}
