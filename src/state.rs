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
    /// The counter's texture at full opacity. The fade is a tint, so this is
    /// built once per coin count rather than once per frame.
    pub hud_coin_base_img:      Option<std::sync::Arc<RgbaImage>>,

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
    /// Which boss the current (or next) fight is; selected from `boss_index`.
    pub boss_kind: crate::constants::BossKind,
    /// Per-part HP/shield/weakpoint state for multi-part bosses (empty for the
    /// single-body Sun Devourer, which uses the scalar `boss_hp` path).
    pub boss_parts: Vec<crate::constants::BossPart>,
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
    /// Ticks left of each generator's tether snapping back into the dome
    /// after it dies (aligns with boss_generators). 0 = not snapping.
    pub boss_generator_snap: Vec<i32>,
    /// Ticks left of each generator's hit flash. 0 = none.
    pub boss_generator_flash: Vec<i32>,
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
    /// Flare Titan: remaining ticks of the post-flare weakpoint window (core
    /// vents; the only window the boss can be hurt, and only with Solar Charge).
    pub boss_flare_window_ticks: u32,
    /// Gravity Weaver: countdown to the next inversion; the telegraph runs
    /// over its last `WEAVER_FLIP_TELEGRAPH` ticks.
    pub weaver_flip_ticks: u32,
    /// Gravity Weaver: ticks every live part stays open after a flip.
    pub weaver_flip_open: u32,
    /// Gravity Weaver: the spindles' orbit angle.
    pub weaver_orbit: f32,
    /// Gravity Weaver: the world is currently upside down. Mirrors
    /// `gravity_dir < 0` while the fight owns it; cleared by every exit.
    pub weaver_inverted: bool,
    /// Gravity Weaver: flips so far this fight (telemetry).
    pub weaver_flips: u32,
    /// Gravity Weaver gauntlet: 0 off, 1 lining up, 2.. shooting (shot
    /// `weaver_gauntlet - 2`), `2 + WEAVER_GAUNTLET_SHOTS` drifting back.
    pub weaver_gauntlet: u8,
    pub weaver_gauntlet_ticks: u32,
    /// Which gauntlets have run: bit 0 after the first pair, bit 1 at the
    /// last spindle. Each runs once.
    pub weaver_gauntlet_done: u8,
    /// The two posts' spindles (the same index twice when one is left).
    pub weaver_gauntlet_parts: [usize; 2],
    /// Height the posts were set at (the player's, when it began).
    pub weaver_gauntlet_y: f32,
    /// The current shooter has fired (and flipped) this shot.
    pub weaver_gauntlet_fired: bool,
    /// Flare Titan: its clock — 0 calm, 1 kindle, 2 flare, 3 vent — and the
    /// ticks left in that phase.
    pub titan_clock: u8,
    pub titan_clock_ticks: u32,
    /// Flare Titan: the vents' orbit angle.
    pub titan_orbit: f32,
    /// Flare Titan: flares so far this fight (telemetry).
    pub titan_flares: u32,
    /// Flare Titan: the player has taken Solar Charge from the current flare.
    pub titan_charged: bool,
    /// Flare Titan: ticks to the next heart an unsheltered player loses in
    /// the flare.
    pub titan_burn_timer: u32,
    /// Magnetar: its clock — 0 beams, 1 pulse count-in, 2 pulse, 3 starquake
    /// — and the ticks left in that phase.
    pub magnetar_clock: u8,
    pub magnetar_clock_ticks: u32,
    /// Magnetar: the beams' own cycle inside the beam phase — 0 dark, 1
    /// charging, 2 live — its ticks left, and cycles run this phase.
    pub magnetar_beam: u8,
    pub magnetar_beam_ticks: u32,
    pub magnetar_beam_cycles: u8,
    /// Magnetar: which beams have already struck this live phase (bits).
    pub magnetar_beam_hit: u8,
    /// Magnetar: the poles' axis angle (radians) and which way it turns.
    pub magnetar_angle: f32,
    pub magnetar_spin: f32,
    /// Magnetar: the next pulse pulls (else it pushes); pulses so far.
    pub magnetar_pull_next: bool,
    pub magnetar_pulses: u32,
    /// Conductor: frames to the next beat of the bar.
    pub boss_beat_ticks: u32,
    /// Conductor: frames per beat (BPM-derived).
    pub boss_beat_interval: u32,
    /// Conductor: Resonance stacks (release on-beat 3 times to arm the weakpoint).
    pub boss_resonance: u32,
    /// Conductor: was the player hooked last tick (release edge detection).
    pub boss_was_hooked: bool,
    /// Conductor: frames until a release stops counting as on-beat.
    pub boss_release_window: u32,
    /// Colossus pattern director: counted down each tick; while > 0 no part may
    /// begin a new telegraph, so simultaneous attacks are spaced out.
    pub boss_pattern_cooldown: u32,
    /// Colossus: while > 0, the whole body is held still (no sway/bob) after the
    /// torso summons a meteor burst, until the meteors have fired and cleared.
    pub boss_meteor_lock_ticks: u32,
    // ── Serpent ──────────────────────────────────────────────────────────
    /// The head's recent positions, newest FIRST, with the arc length travelled
    /// to reach each one. The body samples this history at fixed spacing, so
    /// every segment retraces the exact path the head took — which is what
    /// makes it slither rather than wobble on a shared sine.
    pub serpent_trail: Vec<(f32, f32, f32)>,
    /// Total arc length the head has travelled this fight, so a segment's place
    /// on the body is a distance rather than an index into a shifting buffer.
    pub serpent_arc: f32,
    /// Where the head is steering, in world space. Re-picked when reached.
    pub serpent_goal: (f32, f32),
    /// Position of the shield band along the body, in segments from the head.
    /// Wraps, so the band runs the length of the body over and over.
    pub serpent_band: f32,
    /// What the Serpent is doing, and how long it has been doing it.
    pub serpent_act: SerpentAct,
    pub serpent_act_ticks: u32,
    /// Ticks until it may commit to a new act.
    pub serpent_cooldown: u32,
    /// Rift strike / gambit sites: (x, y, ticks until it erupts).
    pub serpent_rifts: Vec<(f32, f32, u32)>,
    /// The rift the body is currently passing through, if any. Pieces near it
    /// are inside it and are not drawn.
    pub serpent_hole: Option<(f32, f32)>,
    /// How many times the serpent has surfaced this rift sequence.
    pub serpent_surfaced: u32,
    /// The rift that is OPENING while the body is still sinking into the
    /// current one. Drawn charging, so the exit is visibly there before
    /// anything comes out of it.
    pub serpent_next_hole: Option<(f32, f32)>,
    /// Where the coil closes on: the player's position when the act began.
    pub serpent_coil_at: (f32, f32),
    /// Which stage of a burrow the serpent is in, and how long it has been
    /// there. Stages end on what they are waiting for, not on a clock.
    pub serpent_rift_phase: RiftPhase,
    pub serpent_rift_ticks: u32,
    /// Ticks left in the player's reaction window after a gambit capture. The
    /// bite lands at zero unless they have tethered by then.
    pub serpent_gambit_react: u32,
    /// Where a gambit capture spat the player out, and how long that exit
    /// portal is still drawn.
    ///
    /// Deliberately NOT a member of `serpent_rifts`: everything in that list
    /// pulls and captures, and the hole the player just came out of must do
    /// neither, or being taken once means being taken forever. Its being `Some`
    /// is also what marks the gambit as already fired, so the entry holes are
    /// not re-seeded around the player while they are being bitten.
    pub serpent_gambit_exit: Option<(f32, f32)>,
    pub serpent_gambit_exit_ticks: u32,
    /// Where the serpent is wandering to between attacks, and the ticks left
    /// before it gives up on that waypoint and picks another.
    ///
    /// The timeout is not decoration: the head turns at a limited rate, so a
    /// waypoint just inside its turning circle is one it can orbit forever
    /// without ever arriving.
    pub serpent_roam_to: Option<(f32, f32)>,
    pub serpent_roam_ticks: u32,
    /// Grace after a body contact, so a long body cannot drain every heart.
    pub serpent_contact_cooldown: u32,
    /// Head and tail positions when a spine lash began, so the move into
    /// position is a lerp from where they actually were rather than a snap.
    pub serpent_lash_from: Option<((f32, f32), (f32, f32))>,
    /// Where the tail is when launched, and whether it is out.
    pub serpent_tail_out: bool,
    pub serpent_tail_pos: (f32, f32),
    /// Colossus hands: how many attacks the PAIR has committed to. Bumped once
    /// per commitment, not once per hand, so both hands read the same attack.
    pub boss_hand_attack: u32,
    /// Colossus: ticks remaining on the clap's expanding force wave, and where
    /// it came from. Zero when no wave is live.
    pub boss_clap_wave: u32,
    pub boss_clap_at: (f32, f32),
    /// Colossus core vent: ticks of immunity left after a spoke connected.
    pub boss_vent_hit_cooldown: u32,
    /// Colossus torso: how many attacks it has committed to this fight. Decides
    /// which attack comes next via `torso_attack_for`, and — because it is only
    /// bumped when a new telegraph starts — it still names the attack that just
    /// happened while the torso recovers, which is what gates the vulnerability
    /// window.
    pub boss_torso_attack: u32,
    /// Colossus meteor storm: meteors waiting to launch, as (ticks remaining,
    /// incoming angle in degrees). Drained one at a time so the storm is a
    /// sequence rather than a simultaneous burst.
    pub boss_meteor_queue: Vec<(u32, f32)>,
    /// Contact-rule cooldown for the Colossus: after one body-contact heart loss,
    /// the player gets a short grace before it can trigger again, so lingering on
    /// the boss cannot drain every heart in a couple of frames.
    pub boss_contact_cooldown: u32,

    // ── Conductor ────────────────────────────────────────────────────────────
    /// How far through the current beat, 0..1.
    ///
    /// The beat is advanced by REAL elapsed time rather than by counting
    /// ticks. Counting ticks assumes every tick is 1/60s, so a dropped frame
    /// makes the beat fall permanently behind the music — and on a boss scored
    /// against a +/-100ms window, a drifting clock eventually marks correct
    /// releases as misses with nothing to resynchronise it.
    pub conductor_beat_phase: f32,
    /// When the beat clock was last advanced.
    pub conductor_beat_clock: Option<std::time::Instant>,
    /// Which beat of the bar just landed, 0..CONDUCTOR_BEATS_PER_BAR-1.
    ///
    /// The bar is the fight's attack cadence as well as its rhythm, so the
    /// attack director reads this rather than keeping a second timer that could
    /// drift out of phase with the music.
    pub conductor_beat_in_bar: u32,
    /// Which bar of the 8-bar music loop is playing, 0..CONDUCTOR_BEATMAP_BARS-1.
    ///
    /// Scoring targets are the KICK DRUM hits, and which sixteenths are kicks
    /// changes bar to bar — so the fight has to know where in the loop the song
    /// is, not merely where in the bar. Advanced on the downbeat alongside
    /// `conductor_beat_in_bar`, from the same clock, so the two cannot disagree.
    pub conductor_loop_bar: u32,
    /// Scoring releases landed in the current bar. The per-bar drain reads it:
    /// a bar in which the player hit nothing costs a stack, a bar in which they
    /// hit anything does not.
    pub conductor_bar_hits: u32,
    /// Global sixteenth index (bar * 16 + slot) of the last kick scored, or -1.
    ///
    /// One kick can only be claimed once. Without this, two releases either
    /// side of the same kick both land inside its window and score twice, which
    /// rewards exactly the mashing the kick map exists to stop.
    pub conductor_last_kick: i32,
    /// Ticks until the queued wave warning sound plays, or 0.
    ///
    /// The warning is quantised to the off-beat rather than played the instant
    /// a volley is chosen, so it lands in the music instead of across it.
    pub conductor_cue_delay: u32,
    /// Ticks until the player-impact sound may play again.
    ///
    /// The impact sound used to be the HOOK GRAB sound, fired every time the
    /// player attached to a node — which in a game about grabbing and
    /// releasing constantly is several times a second, at nearly four times
    /// the volume of the music. It now marks actually hitting something, and
    /// this stops a multi-contact bounce firing it once per frame.
    pub impact_sfx_cd: u32,
    /// Index into `CONDUCTOR_WAVE_SHAPES` for the running volley.
    pub conductor_wave_shape: u8,
    /// The Conductor's own RNG stream, advanced once per attack decision.
    ///
    /// Its choices used to be rolled from `boss_phase`, which the Sun Devourer
    /// and the Serpent advance but the Conductor NEVER DOES — it is set to 0.0
    /// when the arena is built and left there. Every roll was therefore `0`,
    /// which meant the sustain and crescendo attacks never fired once in the
    /// whole fight and every volley was a single wave. Three attacks and four
    /// volley shapes existed, were telegraphed, tested and drawn, and the
    /// player only ever saw one of each.
    pub conductor_rng: u64,
    /// Level of each of the guard's four quadrant arcs, 0..1.
    pub conductor_eq: [f32; 4],
    /// Ticks until the guard may hit the player again.
    pub conductor_eq_cd: u32,
    /// Whether the core is currently showing its vulnerable sprite.
    ///
    /// Tracked so the image is swapped on the TRANSITION only. Setting it every
    /// frame would rebuild and re-upload the texture sixty times a second for a
    /// picture that did not change.
    pub conductor_core_vuln: bool,
    /// The boss's FULL health for this fight, for the HP bar.
    ///
    /// The bar used to divide by `BOSS_MAX_HP`, a global constant. That is
    /// right for the bosses whose health IS that constant, and wrong for the
    /// part-based ones — the Serpent's health is the sum of its pieces, which
    /// starts at 17, so its bar opened the fight already 15% drained. Recorded
    /// when the fight starts, from whatever the boss actually has.
    pub boss_hp_max: i32,
    /// Tether nodes currently showing the scoring-beat cue.
    ///
    /// Tracked so the cue can be cleared from a node that stops qualifying —
    /// a pooled node handed back and re-issued elsewhere would otherwise carry
    /// the effect with it.
    pub conductor_beat_fx: Vec<String>,
    /// Live hit bursts: (object id, ticks left, drawn size, colour, taken).
    ///
    /// A pool rather than an effect attached to the part that was hit: an
    /// object carries only ONE mega effect, and a boss part is usually already
    /// carrying its state marker when it takes a hit. Attaching the burst to
    /// the part would silently replace the "vulnerable" reticle at the exact
    /// moment the player is looking for confirmation.
    /// (id, ticks left, drawn size, colour, kind) where kind is
    /// `IMPACT_KIND_*`. One pool serves hit bursts and the player's own
    /// beat feedback: both are short-lived effects at a world position, and a
    /// second pool would be the same code with a different name.
    pub impact_live: Vec<(String, u32, f32, (f32, f32, f32), u32)>,
    /// Hit-stop after a landed hit: frames left, where the player hangs,
    /// the rebound they leave with, and the part that shakes meanwhile.
    pub hitstop_ticks: u32,
    pub hitstop_pos: (f32, f32),
    pub hitstop_vel: (f32, f32),
    pub hitstop_target: String,
    pub impact_free: Vec<String>,
    /// Which attack is running: 0 none, 1 bar line, 2 sustain, 3 crescendo.
    pub conductor_attack: u8,
    /// Ticks the running attack has been going, or the telegraph has been up.
    pub conductor_attack_ticks: u32,
    /// Ticks of telegraph left before the queued attack fires. While non-zero
    /// the tell is up and the attack itself has not started.
    pub conductor_telegraph: u32,
    /// Bar-line sweep: +1 sweeps left-to-right, -1 right-to-left.
    pub conductor_bar_dir: f32,
    /// World Y of the centre of the bar line's safe gap.
    pub conductor_bar_gap_y: f32,
    /// Tether nodes currently wearing the sonic-ring effect.
    ///
    /// A node has ONE attached-effect slot, shared with the buff aura, so the
    /// two systems have to agree about who owns it: `tick_buff_node_elec`
    /// skips anything listed here, and this list is released unconditionally
    /// when the attack ends. Without that the wave's rings would be erased by
    /// the buff aura on the next frame, or survive on a recycled node.
    pub conductor_ring_fx: Vec<String>,
    /// How many waves this bar-line attack is bringing, 1..=CONDUCTOR_WAVE_MAX.
    pub conductor_wave_count: u8,
    /// Per-wave strike latch, one bit each.
    ///
    /// A bitmask rather than a bool: with several waves in flight each one may
    /// strike at most once, but a later wave must still be able to catch a
    /// player who survived the first. One shared flag would make every wave
    /// after the first harmless.
    pub conductor_wave_hit: u8,
    /// Whether the current bar-line sweep has already hit the player.
    ///
    /// A sweep crosses the arena over 96 ticks while the contact cooldown is
    /// 45, so without this a player who misses the gap is struck twice by one
    /// wave. Measured: four hearts gone in eleven seconds. One mistake should
    /// cost one heart.
    pub conductor_bar_hit: bool,
    /// Colossus: after a part is destroyed the whole boss is briefly invulnerable,
    /// so the player can't chain-kill two parts within the same second.
    pub boss_part_invuln_ticks: u32,
    /// Whether the SOLAR system is the one currently using the player's single
    /// attached-effect slot. The buff aura uses the same slot, so whichever
    /// system did not attach must not clear.
    pub shield_player_fx: bool,
    /// Whether the BUFF aura currently owns the player's effect slot.
    ///
    /// That slot holds one effect and two systems want it — the buff aura and
    /// the solar shield's dome. Each records what it attached so it only ever
    /// releases its own.
    pub buff_player_fx: bool,
    /// Node ids currently wearing an attached solar-shelter dome. Same reason
    /// as `buff_fx_attached`: an attached effect lives on the object until it
    /// is cleared, so the set has to be remembered to release it.
    pub shield_fx_attached: Vec<String>,
    /// Node ids currently wearing an attached buff aura. An attached mega
    /// sprite lives on the object until it is cleared, so the set that had one
    /// last frame has to be remembered in order to clear the ones that stopped
    /// qualifying — otherwise a recycled pool slot keeps the aura of whatever it
    /// used to be.
    pub buff_fx_attached: Vec<String>,
    /// Colossus gaze beam: live little explosion pops along the telegraphed path
    /// (id, x, y, ticks remaining), which grow out of the path line as the beam
    /// sweeps across it.
    pub beam_explode_live: Vec<(String, f32, f32, u32)>,

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
    pub run_heart_refill_buys: u32,
    pub run_magnet_buys:    u32,
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
    /// Countdown to the next node-light / shadow-caster refresh. The scan is
    /// O(n log n) with a sort; at 60 Hz it dominated the eclipse's frame cost.
    pub eclipse_light_timer: u32,
    /// Reused scratch buffer for that scan, so the refresh does not allocate.
    pub eclipse_node_buf: Vec<(f32, f32, f32)>,

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

impl State {
    /// Whether the boss-damage buff is actually in force.
    ///
    /// The buff is two fields — `player_buff` says which one, `buff_timer` says
    /// how long is left — and every consumer used to test `player_buff > 0` on
    /// its own. Only the code that ticks it down knew about the timer, so when
    /// the buff lapsed it stopped being drawn while remaining in force
    /// mechanically: the Colossus went on absorbing hits into an expired buff
    /// and the player stopped taking damage.
    ///
    /// Reading it through here means the two cannot be believed separately.
    /// Pair it with `retire_buff` for the write side.
    /// Above this the player has "fallen off the top" while gravity is
    /// inverted. Outside an arena a rift inverts gravity with the ground at
    /// the bottom of the screen, so the old fixed -150 stands; in an arena
    /// the play area reaches thousands of pixels up, and the same margin
    /// the floor has under the bottom row of nodes goes above the top row.
    pub fn fall_ceiling_y(&self) -> f32 {
        if self.boss_active {
            crate::constants::BOSS_ARENA_NODE_Y_BOT - crate::constants::WEAVER_CEILING_MARGIN
        } else {
            -150.0
        }
    }

    pub fn buff_active(&self) -> bool {
        buff_is_active(self.player_buff, self.buff_timer)
    }

    /// End the buff and everything that belongs to it.
    ///
    /// `buff_absorbs` goes too, or the next buff starts holding the previous
    /// one's leftover shield.
    pub fn retire_buff(&mut self) {
        self.player_buff = 0;
        self.buff_timer = 0;
        self.buff_absorbs = 0;
    }
}

/// The buff predicate itself, free of `State` so it can be tested directly.
///
/// Both halves matter: `player_buff` alone is what every consumer used to ask,
/// and a stale one outlived its timer.
pub fn buff_is_active(player_buff: u8, buff_timer: u32) -> bool {
    player_buff > 0 && buff_timer > 0
}
