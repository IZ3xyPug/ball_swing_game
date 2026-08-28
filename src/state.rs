use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use crate::constants::*;
use image::RgbaImage;
use quartz::AnimatedSprite;
use crate::poisson::PoissonSampler;

// ── Gravity cannon phase tracking ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum CannonState {
    Idle,
    /// Pre-capture pulse while player is held: 8→7→6→7→8.
    Capturing { seq_idx: usize, frame_timer: u32 },
    /// Player frozen inside barrel; cannon slowly rotates CW.
    Charging { ticks: u32 },
    /// Waiting for the player to accept fast-travel (press F) or let the
    /// default launch fire. Player is held frozen in the barrel.
    WaitingChoice { ticks: u32 },
    /// Fire windup animation: frames 8→0, launch at frame 0.
    FiringDown { frame_idx: usize, frame_timer: u32 },
    /// Post-launch return animation: frames 0→8 before rotation recovery.
    FiringUp { frame_idx: usize, frame_timer: u32 },
    /// Returning to default rotation.
    Recovering { ticks: u32 },
}

#[derive(Clone, Debug)]
pub struct CannonPhase {
    pub id:        String,
    pub state:     CannonState,
    /// Base Y before bob offset.
    pub base_y:    f32,
    /// Phase offset for sin bob (randomised per cannon).
    pub bob_phase: f32,
    /// Current visual rotation in degrees.
    pub rotation:  f32,
    /// True while gravity is flipped (world mirrored vertically). The cannon
    /// barrel points the opposite way and its default rotation is +180°.
    pub flipped:   bool,
}

pub fn lcg(s: &mut u64) -> f32 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let hi = (*s >> 32) as u32;
    (hi as f32) / (u32::MAX as f32)
}

pub fn lcg_range(s: &mut u64, lo: f32, hi: f32) -> f32 { lo + lcg(s) * (hi - lo) }

#[derive(Clone)]
pub struct HookSpec { pub x: f32, pub y: f32 }

/// Tracks a single in-flight spawn-build animation.
/// Objects drop in from above and ease-rotate to their final position.
#[derive(Clone, Debug)]
pub struct SpawnAnim {
    pub id:               String,
    /// Final (resting) top-left position.
    pub target_x:         f32,
    pub target_y:         f32,
    /// World-Y at animation start (above screen).
    pub start_y:          f32,
    /// Starting rotation offset (degrees), eases toward target_rot.
    pub start_rot:        f32,
    pub target_rot:       f32,
    pub elapsed:          u32,
    pub total:            u32,
    /// Restore `is_platform = true` when animation completes (for pads).
    pub restore_platform: bool,
    /// False until the object is near the viewport — animation waits here.
    pub started:          bool,
    /// rotation_momentum to restore when animation starts (0.0 = no change).
    pub restore_rotation_momentum: f32,
}

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

/// Tracks a pending comet: warning display + reserved comet object.
#[derive(Clone)]
pub struct CometWarn {
    /// ID of the warning indicator game object.
    pub warn_obj_id: String,
    /// ID of the comet game object (reserved but invisible until warning ends).
    pub comet_id: String,
    /// Ticks elapsed since warning started.
    pub timer: u32,
    /// Random horizontal offset from player centre to far-above spawn point.
    pub h_offset: f32,
    /// Random vertical distance above player to comet spawn point.
    pub v_offset: f32,
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
    pub rightmost_x: f32,
    /// Tracks how far ahead features have been generated (may be well ahead of
    /// rightmost_x).  Passed in/out of gen_hook_batch so features are not
    /// regenerated over the same X range.
    pub gen_head_x:  f32,
    /// Y cursor for the hop-based generator. Tracks the Y of the last generated
    /// hook so the next batch continues from the correct position.
    pub gen_head_y:  f32,
    /// Position of the most recently *placed* grab point — the value after the
    /// spawner's hazard-avoidance passes, which is not the value the generator
    /// proposed. Every reach check has to be made against this, not against
    /// `gen_head_*`: those two used to drift apart with nothing reconciling
    /// them, so the generator computed each hop from a node that did not exist
    /// at the height it thought it did.
    pub last_hook_x: f32,
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
    pub flip_magnet_locked: Vec<String>,

    pub score_x2_live:      Vec<String>,
    pub score_x2_free:      Vec<String>,
    pub score_x2_rightmost: f32,
    pub score_x2_timer:     u32,
    pub score_x2_magnet_locked: Vec<String>,

    pub zero_g_live:      Vec<String>,
    pub zero_g_free:      Vec<String>,
    pub zero_g_rightmost: f32,
    pub zero_g_timer:     u32,
    pub zero_g_magnet_locked: Vec<String>,

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

    pub dark_mode: bool,
    pub god_mode: bool,
    pub glow_flashes: Vec<(String, u8)>,
    /// One-shot playback for tech_bounce pad impact animation.
    /// Tuple: (pad_id, current_frame_idx, ticks_until_next_frame).
    pub pad_bounce_anim: Vec<(String, usize, u32)>,

    /// Active spawn-build animations (drop-in from above).
    pub spawn_animations: Vec<SpawnAnim>,

    // ── HUD dirty-tracking ──────────────────────────────────────────────
    pub hud_last_dist_fill:     u32,   // dist_fill * 1000 as u32
    pub hud_last_coins:         u32,
    pub hud_last_py:            i32,
    pub hud_last_px:            i32,
    pub hud_last_flip_timer:    u32,
    pub hud_last_zero_g_timer:  u32,
    pub hud_last_score_x2_timer: u32,
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
    /// Player X at the moment they entered space — restored on return.
    pub space_entry_px:          f32,
    /// Set when oxygen runs out: eject back to the surface (extraction) rather
    /// than a hard death.
    pub space_extract:           bool,

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

    pub space_asteroid_live:     Vec<String>,
    pub space_asteroid_free:     Vec<String>,
    pub space_asteroid_rightmost: f32,

    // Space oxygen pickups (extend the oxygen meter)
    pub space_oxygen_pickup_live:      Vec<String>,
    pub space_oxygen_pickup_free:      Vec<String>,
    pub space_oxygen_pickup_rightmost: f32,

    // HUD dirty for oxygen
    pub hud_last_oxygen:         u32,

    // ── Space stasis (entry/exit orbit pause) ─────────────────────────────────
    /// True while the player is in orbit stasis (entry or exit).
    pub space_stasis_active:    bool,
    /// ID of the hook the player is orbiting during space stasis.
    pub space_stasis_hook_id:   String,
    /// True = entry stasis (inside space), false = exit stasis (back in normal zone).
    pub space_stasis_is_entry:  bool,

    // ── Red (arc) coins in space ──────────────────────────────────────────────
    pub space_blue_coin_live:    Vec<String>,
    pub space_blue_coin_free:    Vec<String>,
    pub space_red_coin_live:     Vec<String>,
    pub space_red_coin_free:     Vec<String>,
    /// Coins collected during this space visit — not re-spawned until next entry.
    pub space_coin_spent:        Vec<String>,
    pub space_blue_coin_spent:   Vec<String>,
    pub space_red_coin_spent:    Vec<String>,

    // ── Space gwell pulsing timers ────────────────────────────────────────────
    /// (id, ticks_remaining, is_active) — mirrors normal gwell_timers for space
    pub space_gwell_timers:      Vec<(String, u32, bool)>,
    /// Temporary teleport marker lifecycle: (id, ticks_remaining, phase)
    /// phase 0 = blue marker, phase 1 = dormant marker.
    pub space_bh_teleport_fx:    Vec<(String, u32, u8)>,

    // ── Space planet orbit lock ─────────────────────────────────────────────
    /// Planet id currently locking orbit; empty means no orbit lock.
    pub space_orbit_locked_planet: String,
    /// Signed tangential orbit speed (sign encodes CW/CCW).
    pub space_orbit_speed:         f32,

    // ── Solar ceiling async decode ────────────────────────────────────────────
    /// Pixel-derived y-ratio where the solar surface begins (from top of gif).
    pub solar_surface_ratio: f32,
    /// True once the solar animation has been attached to the scene object.
    pub solar_anim_loaded: bool,
    /// Set on first enter_space: background thread stores the decoded
    /// AnimatedSprite here; tick_solar_pending swaps it onto the object.
    pub solar_anim_pending: Option<Arc<Mutex<Option<AnimatedSprite>>>>,

    // ── Passive-score dead-block system ───────────────────────────────────────
    /// 5000-px block index the player is currently occupying (floor(px/5000)).
    pub score_active_block: i32,
    /// Ticks spent continuously in `score_active_block` without pause.
    pub score_block_ticks: u32,
    /// Blocks where passive time-score is permanently exhausted.
    pub score_dead_blocks: HashSet<i32>,

    // ── Player ball animation ─────────────────────────────────────────────────
    pub player_ball_frame: usize,
    pub player_ball_hit_rewind: bool,
    pub player_ball_frame_timer: u32,

    // ── Gravity cannon obstacle ───────────────────────────────────────────────
    pub cannon_live:       Vec<String>,
    pub cannon_free:       Vec<String>,
    pub cannon_rightmost:  f32,
    pub cannon_phases:     Vec<CannonPhase>,
    /// True while a cannon has captured the player.
    pub cannon_captured:   bool,
    /// ID of the cannon currently holding the player.
    pub cannon_capture_id: String,
    /// Remaining ticks of reduced gravity after cannon launch.
    pub cannon_damp_timer: u32,
    /// True while the player is captured and the fast-travel prompt is shown.
    pub cannon_ft_prompt: bool,
    /// True once the player accepted fast-travel (spends the coin cost).
    pub cannon_ft_active: bool,
    /// Ticks of no-grab grace after fast-travel arrival.
    pub cannon_fast_travel_grace: u32,
    // ── Boss fight ────────────────────────────────────────────────────────────
    pub boss_active: bool,
    pub boss_entry_ticks: u32,      // counts up after crossing threshold
    pub boss_spawned: bool,         // body object made visible
    pub boss_cleared: bool,         // arena cleared on entry (one-shot)
    /// Once the pre-portal approach grapple nodes have been placed.
    pub boss_approach_nodes_spawned: bool,
    /// True while the player orbits a safe node after teleporting into the arena,
    /// before the battle activates. Cleared when the player tethers to a node.
    pub boss_stasis_active: bool,
    /// Orbit angle driver during boss stasis.
    pub boss_stasis_ticks: u32,
    /// The safe node the player orbits during boss stasis (for reference).
    pub boss_stasis_hook: String,
    pub boss_hp: i32,
    pub boss_phase: f32,            // lissajous phase angle (radians, advances per tick)
    pub boss_vx: f32,               // kept for bolts; movement now parametric
    pub boss_vy: f32,
    pub boss_shoot_timer: u32,      // ticks until next bolt
    pub boss_bolt_live: Vec<(String, f32, f32, u32)>, // (id, vx, vy, ttl)
    pub boss_bolt_free: Vec<String>,
    pub boss_asteroids: Vec<String>, // decorative asteroids in the arena
    pub hud_last_boss_hp: i32,
    /// Ticks until the next darkness attack (cooldown).
    pub boss_dark_cooldown: u32,
    /// Remaining ticks of the current darkness phase.
    pub boss_dark_ticks: u32,
    /// True while a darkness phase is active.
    pub boss_dark_active: bool,
    // ── Last-boss barrier / generators / bait-and-bail ──────────────────────
    /// IDs of the generator nodes powering the barrier.
    pub boss_generators: Vec<String>,
    /// Remaining HP per generator (aligns with boss_generators).
    pub boss_generator_hp: Vec<i32>,
    /// True while the protective barrier is up (blocks the sun).
    pub boss_barrier_up: bool,
    /// True once all generators are down — the final (bait-and-bail) phase.
    pub boss_final_phase: bool,
    /// Countdown to the boss's next desperation lunge (final phase).
    pub boss_lunge_telegraph: u32,
    /// Remaining ticks of the active lunge.
    pub boss_lunge_ticks: u32,
    /// World position the boss is lunging toward.
    pub boss_lunge_target: (f32, f32),

    // ── Comets ────────────────────────────────────────────────────────────────
    /// Live comets: (id, vx, vy, ticks_remaining)
    pub comet_live: Vec<(String, f32, f32, u32)>,
    pub comet_free: Vec<String>,
    /// Pending warnings before comet spawn: see CometWarn.
    pub comet_warn_live: Vec<CometWarn>,
    pub warn_free: Vec<String>,
    /// Countdown to next auto-comet spawn attempt (ticks).
    pub comet_spawn_timer: u32,

    // ── Hearts / checkpoint respawn ───────────────────────────────────────────
    /// Hearts remaining this run. Falling costs one; zero ends the run.
    pub hearts: i32,
    /// Hearts a fresh run starts with.
    pub max_hearts: i32,
    /// Last auto-progress checkpoint (a grab-node centre).
    pub checkpoint_x: f32,
    pub checkpoint_y: f32,
    /// Block index the checkpoint was saved for (floor(px / CHECKPOINT_INTERVAL)).
    pub checkpoint_block: i32,
    /// True while the player is in a respawn orbit-in.
    pub respawn_active: bool,
    /// Ticks since respawn started (drives the prompt / wait).
    pub respawn_ticks: u32,
    /// Active buff type from a buff tether node (0 = none).
    pub player_buff: u8,
    /// Remaining ticks of the current buff.
    pub buff_timer: u32,
    /// True for a short window right after a buffed weakpoint hit (hit feedback).
    pub buff_hit_flash: u32,
    /// How many boss projectiles the current buff can absorb before it ends.
    pub buff_absorbs: u32,
    // ── Roguelike upgrade nodes ──────────────────────────────────────────────
    pub upgrade_live:       Vec<String>,
    pub upgrade_free:       Vec<String>,
    pub upgrade_rightmost:  f32,
    /// Oxygen drain multiplier (1.0 normally; < 1.0 with "controlled breathing").
    pub oxygen_drain_scale: f32,
    /// Fractional accumulator so scaled oxygen drain can be non-integer.
    pub oxygen_drain_accum: f32,
    /// Owned momentum-cap upgrade.
    pub upgrade_momentum_bonus: bool,
    /// How many times each run-upgrade has been bought THIS run (drives the
    /// escalating cost of the run-persisting upgrades).
    pub run_heart_buys:     u32,
    pub run_breath_buys:    u32,
    pub run_momentum_buys:  u32,
    /// True while the roguelike upgrade choice dialogue is open.
    pub upgrade_dialogue_active: bool,
    /// Id of the upgrade node the dialogue is attached to.
    pub upgrade_dialogue_node: String,
    /// World-space centre where the upgrade dialogue holds the player.
    pub upgrade_hold_x: f32,
    pub upgrade_hold_y: f32,
    /// True after the dialogue closes: the player is held in stasis until they
    /// tether to a hook node.
    pub upgrade_hold_until_tether: bool,
    /// True if this run entered the space zone (used for meta-currency bonus).
    pub space_visited: bool,
    /// True if this run defeated the boss (used for meta-currency bonus).
    pub boss_killed: bool,
    /// Index of the NEXT scheduled fight (0-based). Advances on each victory,
    /// so `mode::boss_trigger_distance` can drive a whole run of them.
    pub boss_index: u32,
    /// Distance the player had travelled when the current fight began, so they
    /// resume the level exactly where they left it rather than at the arena.
    pub boss_return_x: f32,
    pub boss_return_y: f32,
    /// Coins banked during the current space visit (lost if oxygen runs out).
    pub space_coins_collected: u32,
    /// Solar flare hazard: ticks until the next flare.
    pub flare_cooldown: u32,
    /// Remaining telegraph ticks before a flare erupts.
    pub flare_warn: u32,
    /// True while a flare is actively erupting (damage window).
    pub flare_active: bool,
    /// Remaining ticks of the active flare window.
    pub flare_active_ticks: u32,
    /// Ticks until the next damage application inside an active flare. Damage
    /// is a cadence across the window, not a single check on the eruption
    /// frame, so shelter reached mid-flare saves the remaining ticks.
    pub flare_damage_timer: u32,
    /// World X of the most recently placed shielded node. Shielded nodes are
    /// placed on a fixed DISTANCE cadence rather than a probability roll, so a
    /// flare can never fire into a stretch that has no shelter in it.
    pub last_shield_x: f32,
    /// How many times the chain frontier had to be repaired this run. Should be
    /// zero; a non-zero value means something other than `spawn_hooks` wrote
    /// `rightmost_x` and world generation would have stalled without the guard.
    pub frontier_repairs: u32,

    /// Solar-eclipse approach to a boss: whether it is running and how far
    /// along it is (0 at the far edge, 1 at the teleporter).
    /// Ticks spent held after an upgrade dialogue closed, waiting for the
    /// player to tether out. Bounded so the hold can never trap a run.
    pub upgrade_hold_ticks: u32,

    pub eclipse_active: bool,
    pub eclipse_t: f32,
    /// Objects currently flagged as shadow occluders by the eclipse. Tracked so
    /// teardown is exhaustive — a pooled object left flagged would keep casting
    /// shadows for the rest of the run.
    pub eclipse_shadow_ids: Vec<String>,

    // ── Permanent (meta-bought) upgrades, resolved once at run start ─────────
    // Held as multipliers/counts rather than re-read from the profile every
    // frame: a run should play by the ranks it started with, and the profile
    // is behind a mutex that gameplay has no business locking per tick.
    /// Tether reach multiplier from LONG LINE.
    pub perm_reach_mult: f32,
    /// Top-speed multiplier from FLYWHEEL.
    pub perm_momentum_mult: f32,
    /// Coin pickup radius multiplier from MAGNETISM.
    pub perm_magnet_mult: f32,
    /// Flare damage ticks SUNPROOFING can absorb — refilled at each new flare.
    pub perm_flare_wards: u32,
    pub flare_wards_left: u32,
    /// Checkpoint respawns left that cost no heart (SECOND WIND).
    pub free_respawns_left: u32,
    /// Run telemetry for the flare system, read by the headless harness.
    pub flares_fired: u32,
    pub flare_hearts_lost: u32,
    pub flare_ticks_sheltered: u32,
}
