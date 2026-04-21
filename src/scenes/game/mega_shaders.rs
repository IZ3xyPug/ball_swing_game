// ── mega_shaders.rs — Per-frame mega shader VFX overlays ──────────────────
//
// Pushes MegaShaderSprite instances for:
//   • Player ball: always-on cycling effect (Z/X keys to step back/forward).
//   • Player ball: animated electricity overlay while the air-barrier is active.
//   • Pad hit:     shockwave burst at the bounce point (24-frame decay).
//   • Spinner hit: explosive sparks at the impact point (24-frame decay).
//   • Background:  flaming comet flying across the starfield sky.

use std::sync::{Arc, Mutex};
use quartz::*;
use crate::constants::*;
use crate::state::State;

// ── Effect table ─────────────────────────────────────────────────────────────
//
// Index 0 = no effect (sprite is not pushed at all).
// Indices 1-7 cycle through animated VFX / common effects on the player ball.
//
//   idx | shader_variant | bitmask[0]              | name
//   ----+----------------+-------------------------+-----------------
//     0 | —              | —                       | None
//     1 | 1 (VFX)        | 1 << 0  = 0x0001        | Pulse Glow
//     2 | 1 (VFX)        | 1 << 6  = 0x0040        | Rainbow Shift
//     3 | 1 (VFX)        | 1 << 8  = 0x0100        | Holo Scan
//     4 | 1 (VFX)        | 1 << 13 = 0x2000        | Outline Pulse
//     5 | 1 (VFX)        | 1 << 12 = 0x1000        | Breathing Scale
//     6 | 1 (VFX)        | 1 << 4  = 0x0010        | Glitch
//     7 | 0 (Common)     | 1 << 2  = 0x0004        | Glow (common)

const EFFECT_TABLE: &[(u32, u32)] = &[
    (0, 0),              // 0: None
    (1, 1 << 0),         // 1: Pulse Glow
    (1, 1 << 6),         // 2: Rainbow Shift
    (1, 1 << 8),         // 3: Holo Scan
    (1, 1 << 13),        // 4: Outline Pulse
    (1, 1 << 12),        // 5: Breathing Scale
    (1, 1 << 4),         // 6: Glitch
    (0, 1 << 2),         // 7: Glow (common effects)
];

// ── Window droplet state ──────────────────────────────────────────────────────
//
// Each DropletState represents one water-drop-on-glass sprite.  The pool of 5
// droplets fires independently with randomised timing (1–4 seconds between
// appearances), giving a natural, non-periodic feel.
#[derive(Clone)]
pub struct DropletState {
    /// Screen-space X centre (pixels, 0..VW).
    pub screen_x: f32,
    /// Screen-space Y centre (pixels, 0..VH).
    pub screen_y: f32,
    /// Half-size of the sprite square in screen pixels.
    pub size_px: f32,
    /// Size multiplier forwarded to the shader (tint_color.g).
    pub size_mult: f32,
    /// How many frames this droplet has been alive.
    pub age: u32,
    /// Total frames the droplet lives (3–6 seconds).
    pub lifetime: u32,
    /// Whether the droplet is currently visible.
    pub active: bool,
    /// Frames remaining until the next spawn.
    pub next_delay: u32,
    seed: u64,
}

impl DropletState {
    fn lcg_next(&mut self) -> f32 {
        self.seed = self.seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.seed >> 33) as f32) / (u32::MAX as f32)
    }

    /// Activate this droplet at a fresh random position.
    fn spawn_new(&mut self) {
        // Keep drops 10–90 % from each edge so they don't overlap with the HUD.
        self.screen_x   = (0.10 + self.lcg_next() * 0.80) * VW;
        self.screen_y   = (0.12 + self.lcg_next() * 0.72) * VH;
        // Size: sprite square = 90–180 px half-extent (larger, more visible).
        self.size_px    = 90.0 + self.lcg_next() * 90.0;
        // size_mult controls blob scale inside the shader (0.65–1.20).
        self.size_mult  = 0.65 + self.lcg_next() * 0.55;
        self.age        = 0;
        // Lifetime: 3–7 seconds at 60 fps.
        self.lifetime   = 180 + (self.lcg_next() * 240.0) as u32;
        self.active     = true;
        self.next_delay = 0;
    }

    /// Call once per frame.  Advances age, handles death and re-spawn delay.
    pub fn tick(&mut self, rain_on: bool) {
        if !rain_on {
            // Rain turned off: kill immediately and don't reschedule.
            self.active     = false;
            self.next_delay = 0;
            return;
        }
        if self.active {
            self.age += 1;
            if self.age >= self.lifetime {
                self.active = false;
                // Wait 1–4 seconds before the next drop (60–240 frames).
                self.next_delay = 60 + (self.lcg_next() * 180.0) as u32;
            }
        } else if self.next_delay > 0 {
            self.next_delay -= 1;
        } else {
            self.spawn_new();
        }
    }

    /// Create the initial pool of 5 droplets, staggered so they don't all
    /// appear on the first frame rain is enabled.
    pub fn init_droplets() -> Vec<DropletState> {
        let seeds = [
            0x1A2B_3C4D_5E6F_7A8Bu64,
            0xDEAD_CAFE_BABE_0102u64,
            0x9876_5432_10FE_DCBAu64,
            0x0F0F_F0F0_AAAA_5555u64,
            0x1111_2222_3333_4444u64,
        ];
        seeds.iter().enumerate().map(|(i, &seed)| DropletState {
            screen_x:   0.0,
            screen_y:   0.0,
            size_px:    70.0,
            size_mult:  1.0,
            age:        0,
            lifetime:   0,
            active:     false,
            // Stagger initial delays: droplets fire every ~0.75 s at the start.
            next_delay: (i as u32) * 45 + 30,
            seed,
        }).collect()
    }
}
//
// Comets are SCREEN-SPACE objects: positions are stored in screen pixels and
// converted to UV at push time.  This makes them completely independent of
// camera position and zoom — they always appear as a fixed background layer.
//
// Layout (pushed as two sprites):
//   HEAD  – BIT_PULSE_GLOW | BIT_FIRE, square sprite centred at (screen_x, screen_y)
//   TAIL  – BIT_COMET_TAIL, wide+flat sprite whose LEFT EDGE aligns with the
//           head centre.  Full sprite = tail (head end left, tip right).
const TAIL_MULT: f32 = 7.0;  // tail total length = TAIL_MULT × head diameter

#[derive(Clone)]
pub struct CometState {
    /// Screen-space X position of the head centre (pixels from left, 0..VW).
    pub screen_x: f32,
    /// Screen-space Y position of the head centre (pixels from top, 0..VH).
    pub screen_y: f32,
    /// Horizontal speed in screen pixels per frame (comet moves left).
    pub speed_px: f32,
    /// Head radius in screen pixels.
    pub head_r_px: f32,
    /// Vertical drift per frame (screen pixels; positive = downward).
    pub traj_vy_px: f32,
    /// Frames remaining before re-entering after the previous pass.
    pub delay: u32,
    /// LCG seed for per-comet randomness.
    seed: u64,
}

impl CometState {
    fn lcg_next(&mut self) -> f32 {
        self.seed = self.seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.seed >> 33) as f32) / (u32::MAX as f32)
    }

    /// Call when the comet exits the left edge.  Picks fresh screen-space params.
    pub fn wrap_reset(&mut self) {
        // 3–6 second delay (at 60 fps) before the next pass.
        self.delay       = 180 + (self.lcg_next() * 180.0) as u32;
        // Y: 5–20 % from the top of the screen.
        self.screen_y    = (0.05 + self.lcg_next() * 0.15) * VH;
        // Speed: 3.5–8 px / frame.
        self.speed_px    = 3.5 + self.lcg_next() * 4.5;
        // Head radius: 20–38 px on screen.
        self.head_r_px   = 20.0 + self.lcg_next() * 18.0;
        // Tiny vertical drift.
        self.traj_vy_px  = (self.lcg_next() - 0.5) * 0.15;
        // Re-enter just off the right edge (account for full tail extent).
        self.screen_x    = VW + self.head_r_px * (1.0 + TAIL_MULT * 2.0) + 80.0;
    }

    /// Create the initial set of three staggered comets.
    pub fn init_comets() -> Vec<CometState> {
        vec![
            CometState {
                screen_x: VW * 0.30, screen_y: VH * 0.08,
                speed_px: 5.0, head_r_px: 30.0, traj_vy_px: 0.05,
                delay: 0, seed: 0x1234_5678_9ABC_DEF0,
            },
            CometState {
                screen_x: VW * 0.72, screen_y: VH * 0.14,
                speed_px: 7.0, head_r_px: 22.0, traj_vy_px: -0.04,
                delay: 60, seed: 0xABCD_EF01_2345_6789,
            },
            CometState {
                screen_x: VW * 1.20, screen_y: VH * 0.06,
                speed_px: 4.0, head_r_px: 38.0, traj_vy_px: 0.03,
                delay: 120, seed: 0x5555_AAAA_BBBB_CCCC,
            },
        ]
    }
}

/// Per-frame tick for all mega shader sprite overlays.
/// `ball_img`   – white circle used for player + triggered VFX.
/// `comet_img`  – orange circle used for comet head sprites.
/// `comets`     – mutable comet states (screen-pixel space).
/// `droplets`   – mutable window-droplet states (screen-pixel space).
///
/// Rain state and air-shield electricity toggle are read from canvas vars:
///   `rain_state`       – 0=off, 1=right slant, 2=left, 3=down, 4=strong diagonal
///   `air_shield_elec`  – bool, electricity mode on the air shield arc
pub fn tick_mega_shaders(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    ball_img: &Arc<image::RgbaImage>,
    comet_img: &Arc<image::RgbaImage>,
    comets: &mut Vec<CometState>,
    droplets: &mut Vec<DropletState>,
) {
    // ── Snapshot relevant state (short lock) ─────────────────────────────
    let (mut px, mut py, mut vx, mut vy, pad_timer, pad_pos, spin_timer, spin_pos);
    {
        let mut s = st.lock().unwrap();
        px = s.px;
        py = s.py;
        vx = s.vx;
        vy = s.vy;
        pad_timer  = s.pad_hit_vfx_timer;
        pad_pos    = s.pad_hit_pos;
        spin_timer = s.spinner_hit_vfx_timer;
        spin_pos   = s.spinner_hit_pos;
        // Decrement timers every frame.
        if s.pad_hit_vfx_timer   > 0 { s.pad_hit_vfx_timer   -= 1; }
        if s.spinner_hit_vfx_timer > 0 { s.spinner_hit_vfx_timer -= 1; }
    }

    // ── Use crystalline-corrected position from the engine object ─────────
    if let Some(obj) = c.get_game_object("player") {
        px = obj.position.0 + PLAYER_R;
        py = obj.position.1 + PLAYER_R;
        vx = obj.momentum.0;
        vy = obj.momentum.1;
    }

    // ── Player UV position via world_to_screen ────────────────────────────
    // Using world_to_screen gives the same camera transform the scene pass
    // uses to render the player ball, eliminating the one-frame overlay lag
    // that occurs when camera.position is stale during on_update.
    let (sx, sy)   = c.world_to_screen((px, py));
    let player_uv  = (sx / VW, sy / VH);

    // ── Camera zoom for SCALE calculations ───────────────────────────────
    // We still need zoom to scale player overlay sprites correctly (they
    // should match the player ball visual size at any zoom level).
    let cam_zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0);

    // Convert a world-space size to UV accounting for zoom.
    let ws = |sw: f32, sh: f32| -> (f32, f32) {
        (sw * cam_zoom / VW, sh * cam_zoom / VH)
    };
    // Convert a world-space velocity to UV / frame accounting for zoom.
    let wv = |vx: f32, vy: f32| -> (f32, f32) {
        (vx * cam_zoom / VW, vy * cam_zoom / VH)
    };

    // ── Rain / air-shield-elec canvas var reads ───────────────────────────
    let rain_state = match c.get_var("rain_state") {
        Some(Value::I32(v)) => v.max(0) as u32,
        _ => 0u32,
    };
    let rain_enabled = rain_state > 0;
    let rain_angle: f32 = match rain_state {
        1 => 0.12,   // rightward slant (default)
        2 => -0.12,  // leftward slant
        3 => 0.0,    // straight down
        4 => 0.30,   // strong diagonal
        _ => 0.12,
    };
    let air_shield_elec = matches!(c.get_var("air_shield_elec"), Some(Value::Bool(true)));

    let speed = (vx * vx + vy * vy).sqrt();
    let tint_r = C_PLAYER.0 as f32 / 255.0;
    let tint_g = C_PLAYER.1 as f32 / 255.0;
    let tint_b = C_PLAYER.2 as f32 / 255.0;

    // ── Player: always-on cycling effect ─────────────────────────────────
    let effect_idx = (c.get_i32("player_effect_idx").max(0) as usize)
        .min(PLAYER_EFFECT_COUNT - 1);
    if effect_idx > 0 {
        let (variant, bitmask0) = EFFECT_TABLE[effect_idx];
        if bitmask0 != 0 {
            c.push_mega_sprite(MegaShaderSprite {
                image: ball_img.clone(),
                instance: MegaShaderInstance {
                    world_position: player_uv,
                    scale:          ws(PLAYER_R * 2.0, PLAYER_R * 2.0),
                    rotation: 0.0,
                    tint_color: (tint_r, tint_g, tint_b, 1.0),
                    bitmask: [bitmask0, 0, 0, 0],
                    velocity: wv(vx, vy),
                },
                shader_variant: variant,
            });
        }
    }

    // ── Player: air shield (≥ 2/3 max speed) ────────────────────────────
    //
    // Two mega-shader sprites replace the old post-processing air_barrier slot:
    //
    //   1. AIR SHIELD ARC (BIT_AIR_SHIELD):  the glowing half-circle in front
    //      of the player, sized to outer_r_px = PLAYER_R × 3.5.
    //      Optional ELECTRICITY mode (BIT_AIR_SHIELD_ELEC) adds arcs on ring.
    //
    //   2. ELECTRICITY + OUTLINE PULSE (BIT_ELECTRICITY | BIT_OUTLINE_PULSE):
    //      sparks radiating from the player ball at 4.5× ball size.
    let air_threshold = MOMENTUM_CAP * (2.0 / 3.0);
    if speed >= air_threshold {
        // ── 1. Electricity overlay on the player ball ──────────────────
        c.push_mega_sprite(MegaShaderSprite {
            image: ball_img.clone(),
            instance: MegaShaderInstance {
                world_position: player_uv,
                scale:          ws(PLAYER_R * 4.5, PLAYER_R * 4.5),
                rotation: 0.0,
                tint_color: (0.70, 0.85, 1.0, 1.0),
                bitmask: [(1u32 << 2) | (1u32 << 13), 0, 0, 0],
                velocity: wv(vx, vy),
            },
            shader_variant: 1,
        });

        // ── 2. Air shield arc (screen-space, zoom-corrected) ───────────
        let outer_r_px  = PLAYER_R * 3.5;
        let vlen        = speed.max(0.001);
        let speed_norm  = ((speed - air_threshold) / (MOMENTUM_CAP - air_threshold)).clamp(0.0, 1.0);
        let facing      = (vx / vlen, vy / vlen);  // normalized world direction
        let shield_bit  = if air_shield_elec {
            (1u32 << 17) | (1u32 << 18)
        } else {
            1u32 << 17
        };
        c.push_mega_sprite(MegaShaderSprite {
            image: ball_img.clone(),
            instance: MegaShaderInstance {
                world_position: player_uv,
                scale:          ws(outer_r_px * 2.0, outer_r_px * 2.0),
                rotation: 0.0,
                // tint_color.r = speed_norm, .g = flicker_amount, .b unused, .a = 1
                tint_color: (speed_norm, 0.65, 1.0, 1.0),
                bitmask: [shield_bit, 0, 0, 0],
                velocity: (facing.0, facing.1),   // normalized facing direction
            },
            shader_variant: 1,
        });
    }

    // ── Pad hit: shockwave burst ──────────────────────────────────────────
    //
    // BIT_SHOCKWAVE = bit 7 (animated VFX variant 1).
    // Alpha fades from 1.0 → 0 as the timer runs down.
    if pad_timer > 0 {
        let alpha = pad_timer as f32 / 24.0;
        let (psx, psy) = c.world_to_screen((pad_pos.0, pad_pos.1));
        c.push_mega_sprite(MegaShaderSprite {
            image: ball_img.clone(),
            instance: MegaShaderInstance {
                world_position: (psx / VW, psy / VH),
                scale:          ws(PAD_W * 1.1, PAD_W * 1.1),
                rotation: 0.0,
                tint_color: (0.30, 0.80, 1.0, alpha),
                bitmask: [1u32 << 7, 0, 0, 0],
                velocity: (0.0, 0.0),
            },
            shader_variant: 1,
        });
    }

    // ── Spinner hit: explosive sparks burst ───────────────────────────────
    //
    // BIT_EXPLOSIVE_SPARKS = bit 11 (animated VFX variant 1).
    if spin_timer > 0 {
        let alpha = spin_timer as f32 / 24.0;
        let (spx, spy) = c.world_to_screen((spin_pos.0, spin_pos.1));
        c.push_mega_sprite(MegaShaderSprite {
            image: ball_img.clone(),
            instance: MegaShaderInstance {
                world_position: (spx / VW, spy / VH),
                scale:          ws(SPINNER_W * 0.9, SPINNER_W * 0.9),
                rotation: 0.0,
                tint_color: (1.0, 0.50, 0.15, alpha),
                bitmask: [1u32 << 11, 0, 0, 0],
                velocity: (0.0, 0.0),
            },
            shader_variant: 1,
        });
    }

    // ── Background comets ─────────────────────────────────────────────────
    //
    // Multiple fireballs arc across the starfield sky.  Each CometState
    // ── Background comets (screen-space, two sprites each) ───────────────
    //
    // Each comet pushes two sprites:
    //   HEAD  – BIT_PULSE_GLOW | BIT_FIRE, square sprite at head centre.
    //   TAIL  – BIT_COMET_TAIL, wide+flat sprite; left edge = head centre,
    //           right edge = tail tip.  TAIL_MULT head-diameters long.
    //
    // All positions are in screen pixels, converted to UV at push time by
    // dividing by VW / VH.  Camera position and zoom do NOT affect comets.
    for comet in comets.iter_mut() {
        if comet.delay > 0 {
            comet.delay -= 1;
            continue;
        }

        comet.screen_x -= comet.speed_px;
        comet.screen_y += comet.traj_vy_px;

        // Wrap when the full comet (head + tail) has exited left.
        let tail_total_px = comet.head_r_px * 2.0 * TAIL_MULT;
        if comet.screen_x < -(comet.head_r_px * (1.0 + TAIL_MULT * 2.0) + 10.0) {
            comet.wrap_reset();
            continue;
        }

        // ── HEAD sprite ────────────────────────────────────────────────
        let head_uv_x   = comet.screen_x / VW;
        let head_uv_y   = comet.screen_y / VH;
        let head_scale  = (comet.head_r_px * 2.0 / VW, comet.head_r_px * 2.0 / VH);
        // Velocity in UV / frame for COMET_TAIL direction hint.
        let vel_uv      = (-comet.speed_px / VW, comet.traj_vy_px / VH);

        c.push_mega_sprite(MegaShaderSprite {
            image: comet_img.clone(),
            instance: MegaShaderInstance {
                world_position: (head_uv_x, head_uv_y),
                scale:          head_scale,
                rotation: 0.0,
                tint_color: (0.55, 0.82, 1.0, 1.0),  // blue-white comet head
                bitmask: [(1u32 << 0) | (1u32 << 1), 0, 0, 0],
                velocity: vel_uv,
            },
            shader_variant: 1,
        });

        // ── TAIL sprite ────────────────────────────────────────────────
        // Centre = head centre + head_r + tail_half_px (to the right).
        let tail_center_x = comet.screen_x + comet.head_r_px + tail_total_px * 0.5;
        let tail_scale_x  = tail_total_px / VW;
        let tail_scale_y  = comet.head_r_px * 3.0 / VH;  // 3× head radius tall

        c.push_mega_sprite(MegaShaderSprite {
            image: comet_img.clone(),
            instance: MegaShaderInstance {
                world_position: (tail_center_x / VW, head_uv_y),
                scale:          (tail_scale_x, tail_scale_y),
                rotation: 0.0,
                tint_color: (1.0, 1.0, 1.0, 1.0),  // white — shader colors drive the blue palette
                bitmask: [1u32 << 15, 0, 0, 0],
                velocity: vel_uv,
            },
            shader_variant: 1,
        });
    }

    // ── Full-screen rain overlay ──────────────────────────────────────────
    //
    // A full-screen sprite (UV position 0.5, 0.5; UV scale 1, 1) with
    // BIT_RAIN (bit 16).  velocity.x = rain slant angle (positive = rightward).
    if rain_enabled {
        c.push_mega_sprite(MegaShaderSprite {
            image: ball_img.clone(),
            instance: MegaShaderInstance {
                world_position: (0.5, 0.5),
                scale:          (1.0, 1.0),
                rotation: 0.0,
                tint_color: (0.72, 0.85, 1.0, 0.70),
                bitmask: [1u32 << 16, 0, 0, 0],
                velocity: (rain_angle, 0.0),
            },
            shader_variant: 1,
        });
    }

    // ── Window droplets (appear randomly while rain is enabled) ──────────
    //
    // 5 independent droplets with staggered timing (1–4 s between each).
    // Each is a small square sprite drawn with BIT_WINDOW_DROPLET (bit 19)
    // which renders a translucent water-drop-on-glass shape, optionally
    // with a drip tail that extends downward over time.
    for droplet in droplets.iter_mut() {
        droplet.tick(rain_enabled);
        if droplet.active {
            let age_norm = droplet.age as f32 / droplet.lifetime.max(1) as f32;
            let uv_x     = droplet.screen_x / VW;
            let uv_y     = droplet.screen_y / VH;
            let scale_x  = droplet.size_px * 2.0 / VW;
            let scale_y  = droplet.size_px * 2.0 / VH;
            c.push_mega_sprite(MegaShaderSprite {
                image: ball_img.clone(),
                instance: MegaShaderInstance {
                    world_position: (uv_x, uv_y),
                    scale:          (scale_x, scale_y),
                    rotation: 0.0,
                    // .r = age_norm (0..1), .g = size_mult, .b unused, .a = 1
                    tint_color: (age_norm, droplet.size_mult, 0.0, 1.0),
                    bitmask: [1u32 << 19, 0, 0, 0],
                    velocity: (0.0, 0.0),
                },
                shader_variant: 1,
            });
        }
    }
}
