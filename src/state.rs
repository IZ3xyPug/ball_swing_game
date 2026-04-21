use std::collections::VecDeque;
use crate::constants::*;
use quartz::Timer;

pub fn lcg(s: &mut u64) -> f32 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let hi = (*s >> 32) as u32;
    (hi as f32) / (u32::MAX as f32)
}

pub fn lcg_range(s: &mut u64, lo: f32, hi: f32) -> f32 { lo + lcg(s) * (hi - lo) }

#[derive(Clone)]
pub struct HookSpec { pub x: f32, pub y: f32 }

pub fn gen_hook_batch(seed: &mut u64, from_x: f32, start_y: &mut f32, difficulty: f32) -> VecDeque<HookSpec> {
    let mut hooks = VecDeque::new();
    let mut x = from_x;
    let mut y = *start_y;

    if TEST_LAYOUT_MODE {
        let lanes = [VH*0.24, VH*0.36, VH*0.50, VH*0.64, VH*0.52, VH*0.38];
        for i in 0..MAX_HOOKS_LIVE {
            x += TEST_HOOK_GAP + difficulty * 20.0;
            let lane_idx = ((x / TEST_HOOK_GAP) as usize + i) % lanes.len();
            let target = lanes[lane_idx];
            let blend = 0.58;
            let wobble = lcg_range(seed, -20.0, 20.0);
            y = (y * (1.0 - blend) + target * blend + wobble).clamp(VH*0.12, VH*0.80);
            hooks.push_back(HookSpec { x, y });
        }
        *start_y = y;
        return hooks;
    }

    for _ in 0..MAX_HOOKS_LIVE {
        let gap = lcg_range(seed, (780.0 - difficulty*50.0).max(680.0), 1200.0 + difficulty*160.0);
        let target_y = lcg_range(seed, VH*0.18, VH*0.72);
        let blend = 0.30 + difficulty * 0.12;
        let wobble = lcg_range(seed, -140.0 - difficulty*80.0, 140.0 + difficulty*80.0);
        let mut next_y = y * (1.0 - blend) + target_y * blend + wobble;
        let max_step = 200.0 + difficulty * 100.0;
        next_y = y + (next_y - y).clamp(-max_step, max_step);

        x += gap;
        y = next_y.clamp(VH*0.14, VH*0.76);
        hooks.push_back(HookSpec { x, y });
    }
    *start_y = y;
    hooks
}

#[derive(Clone)]
pub struct State {
    pub px: f32, pub py: f32,
    pub vx: f32, pub vy: f32,

    pub hooked:      bool,
    pub hook_x:      f32,
    pub hook_y:      f32,
    pub rope_len:    f32,
    pub active_hook: String,

    pub distance:   f32,
    pub score:      u32,
    pub coin_count: u32,
    pub gravity_dir: f32,

    pub seed:        u64,
    pub pending:     VecDeque<HookSpec>,
    pub live_hooks:  Vec<String>,
    pub pool_free:   Vec<String>,
    pub gen_y:       f32,
    pub rightmost_x: f32,

    pub dead:  bool,
    pub ticks: u32,

    pub pad_live:      Vec<String>,
    pub pad_free:      Vec<String>,
    pub pad_rightmost: f32,
    pub pad_origins:   Vec<(String, f32, f32, f32, f32)>,
    pub pad_bounce_count: u32,

    pub spinner_live:      Vec<String>,
    pub spinner_free:      Vec<String>,
    pub spinner_rightmost: f32,
    pub spinner_origins:   Vec<(String, f32, f32, f32, f32)>,
    pub spinners_enabled:  bool,
    #[allow(dead_code)]
    pub spinner_spin_enabled: bool,
    pub spinner_hit_cooldown: u8,

    pub coin_live:      Vec<String>,
    pub coin_free:      Vec<String>,
    pub coin_rightmost: f32,
    pub coin_magnet_locked: Vec<String>,
    pub magnet_debug: bool,

    pub flip_live:      Vec<String>,
    pub flip_free:      Vec<String>,
    pub flip_rightmost: f32,
    pub flip_timer:     u32,

    pub score_x2_live:      Vec<String>,
    pub score_x2_free:      Vec<String>,
    pub score_x2_rightmost: f32,
    pub score_x2_timer:     u32,

    pub zero_g_live:      Vec<String>,
    pub zero_g_free:      Vec<String>,
    pub zero_g_rightmost: f32,
    pub zero_g_timer:     u32,

    pub gate_live:      Vec<String>,
    pub gate_free:      Vec<String>,
    pub gate_rightmost: f32,

    pub gwell_live:      Vec<String>,
    pub gwell_free:      Vec<String>,
    pub gwell_rightmost: f32,
    /// Per-well timer tracking: (id, ticks_remaining, currently_active)
    pub gwell_timers:    Vec<(String, u32, bool)>,

    pub turret_live:      Vec<String>,
    pub turret_free:      Vec<String>,
    pub turret_rightmost: f32,
    /// (turret_id, ticks_until_next_shot)
    pub turret_timers:    Vec<(String, u32)>,
    /// (bullet_id, vx, vy, ticks_remaining)
    pub bullet_live:      Vec<(String, f32, f32, u32)>,
    pub bullet_free:      Vec<String>,

    pub bounce_enabled: bool,

    pub dark_mode: bool,
    pub glow_flashes: Vec<(String, u8)>,

    // ── Camera effect cooldowns ─────────────────────────────────────
    pub shake_cooldown: Timer,
    pub flash_cooldown: Timer,

    // ── HUD dirty-tracking ──────────────────────────────────────────────
    pub hud_last_dist_fill:     u32,   // dist_fill * 1000 as u32
    pub hud_last_coins:         u32,
    pub hud_last_momentum:      u32,   // momentum * 10 as u32
    pub hud_last_gravity_flip:  bool,
    pub hud_last_py:            i32,
    pub hud_last_px:            i32,
    pub hud_last_flip_timer:    u32,
    pub hud_last_zero_g_timer:  u32,

    // ── Impact burst emitters (name, remaining frames) ──────────────
    pub burst_emitters: Vec<(String, u8)>,
    pub burst_counter: u32,
    pub hud_last_score:         u32,

    // ── Mega shader VFX timers ───────────────────────────────────────────────
    /// Frames remaining for pad-hit shockwave overlay (counts down to 0).
    pub pad_hit_vfx_timer: u8,
    /// World position of the most recent pad hit (used for shockwave origin).
    pub pad_hit_pos: (f32, f32),
    /// Frames remaining for spinner-hit explosive-sparks overlay.
    pub spinner_hit_vfx_timer: u8,
    /// World position of the most recent spinner hit (used for sparks origin).
    pub spinner_hit_pos: (f32, f32),
}
