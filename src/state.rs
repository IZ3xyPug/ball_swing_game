use std::collections::VecDeque;
use crate::constants::*;
use image::RgbaImage;
use crate::poisson::PoissonSampler;

pub fn lcg(s: &mut u64) -> f32 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let hi = (*s >> 32) as u32;
    (hi as f32) / (u32::MAX as f32)
}

pub fn lcg_range(s: &mut u64, lo: f32, hi: f32) -> f32 { lo + lcg(s) * (hi - lo) }

#[derive(Clone)]
pub struct HookSpec { pub x: f32, pub y: f32 }

pub fn gen_hook_batch(seed: &mut u64, from_x: f32, gen_head_x: &mut f32, gen_head_y: &mut f32, distance_px: f32) -> VecDeque<HookSpec> {
    use crate::level_gen::generate_next_hook;

    // Ensure the generation head starts at least at from_x.
    if *gen_head_x < from_x {
        *gen_head_x = from_x;
    }

    let mut all_hooks: VecDeque<HookSpec> = VecDeque::new();

    // Hop-by-hop: each call produces exactly one hook guaranteed within rope reach.
    while all_hooks.len() < MAX_HOOKS_LIVE {
        let hook = generate_next_hook(seed, gen_head_x, gen_head_y, distance_px);
        all_hooks.push_back(hook);
    }

    all_hooks
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
    pub score_time_awards: u32,
    pub score_distance_awards: u32,

    pub seed:        u64,
    pub pending:     VecDeque<HookSpec>,
    pub live_hooks:  Vec<String>,
    pub pool_free:   Vec<String>,
    pub gen_y:       f32,  // kept for API compatibility; unused by level_gen
    pub rightmost_x: f32,
    /// Tracks how far ahead features have been generated (may be well ahead of
    /// rightmost_x).  Passed in/out of gen_hook_batch so features are not
    /// regenerated over the same X range.
    pub gen_head_x:  f32,
    /// Y cursor for the hop-based generator. Tracks the Y of the last generated
    /// hook so the next batch continues from the correct position.
    pub gen_head_y:  f32,
    /// Y-position of the most recently placed grab point. Used as a safety net
    /// to nudge hooks that land too close vertically after Y clamping.
    pub last_hook_y: f32,

    /// Shared Poisson-disk sampler — tracks all placed pad/spinner centres so
    /// that new placements are organically spaced from existing objects.
    pub world_sampler: PoissonSampler,

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
    pub god_mode: bool,
    pub glow_flashes: Vec<(String, u8)>,

    // ── HUD dirty-tracking ──────────────────────────────────────────────
    pub hud_last_dist_fill:     u32,   // dist_fill * 1000 as u32
    pub hud_last_coins:         u32,
    pub hud_last_momentum:      u32,   // momentum * 10 as u32
    pub hud_last_gravity_flip:  bool,
    pub hud_last_py:            i32,
    pub hud_last_px:            i32,
    pub hud_last_flip_timer:    u32,
    pub hud_last_zero_g_timer:  u32,
    pub hud_last_score:         u32,
    pub hud_coin_fade_ticks:    u32,
    pub hud_coin_alpha:         u8,
    pub hud_last_coin_alpha:    u8,
    pub hud_coin_base_img:      Option<RgbaImage>,

    // ── Space zone ──────────────────────────────────────────────────────
    /// True while player is in the space zone.
    pub in_space_mode:           bool,
    /// Set ONLY by rocket pad collision. Guards the space entry threshold so
    /// no amount of swinging or zero-g can accidentally cross into space.
    pub space_launch_active:     bool,
    /// True once momentum has been zeroed at the settle depth; prevents re-trigger.
    pub space_settle_done:       bool,
    /// Ticks since entering space (used for welcome text).
    pub space_welcome_ticks:     u32,
    /// Oxygen remaining in ticks.
    pub space_oxygen:            u32,
    /// Ticks before forced return after oxygen hits 0 (grace countdown).
    pub space_return_delay:      u32,
    /// Current manually-managed camera Y when in space (world coords).
    pub space_cam_y:             f32,
    /// Background scale frozen at space entry (for parallax starfield effect).
    pub space_entry_bg_scale:    f32,

    // Rocket pads (rare in normal game)
    pub rocket_pad_live:         Vec<String>,
    pub rocket_pad_free:         Vec<String>,
    pub rocket_pad_rightmost:    f32,

    // Space objects (live only while in_space_mode)
    pub space_planet_live:       Vec<String>,
    pub space_planet_free:       Vec<String>,
    pub space_planet_rightmost:  f32,
    /// Per-planet gravity config: (id, gravity_radius, strength)
    pub space_planet_data:       Vec<(String, f32, f32)>,

    pub space_hook_live:         Vec<String>,
    pub space_hook_free:         Vec<String>,
    pub space_hook_rightmost:    f32,

    pub space_coin_live:         Vec<String>,
    pub space_coin_free:         Vec<String>,
    pub space_coin_rightmost:    f32,

    pub space_blackhole_live:    Vec<String>,
    pub space_blackhole_free:    Vec<String>,
    pub space_blackhole_rightmost: f32,
    /// Per-black-hole gravity config: (id, gravity_radius, strength)
    pub space_blackhole_data:    Vec<(String, f32, f32)>,

    // HUD dirty for oxygen
    pub hud_last_oxygen:         u32,
}
