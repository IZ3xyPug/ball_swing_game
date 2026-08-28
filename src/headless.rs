// ── headless.rs — Window-less simulation driver ───────────────────────────────
// Builds the *real* Canvas + game scene, drives the engine tick loop without a
// window, and injects a simple auto-swing policy via keyboard events. It emits
// movement/difficulty metrics so we can "feel" the loop in CI/analysis.
//
// Why this works: `Canvas::new` ignores its prism Context, the game implements
// its own rope physics inside the on_update tick callbacks, and the engine runs
// those callbacks + physics from `OnEvent::on_event(TickEvent)`. None of that
// needs a GPU surface. We simply drive `on_event` directly and never call
// `draw`, so no rendering / light / particle GPU work is emitted.

use quartz::*;
use ramp::prism;
use prism::event::{
    Event, Key, KeyboardEvent, KeyboardState, Modifiers, NamedKey, OnEvent, TickEvent,
};
use prism::drawable::SizedTree;

use crate::constants::*;
use crate::achievements::TOTAL_COINS_COLLECTED_VAR;
use crate::menu::{
    build_tutorial_scene, build_menu_scene, build_gameover_oxygen_scene, build_gameover_scene,
    build_gameover_sun_scene, build_menu_settings_scene, build_achievements_scene,
    build_stats_scene, build_daily_reward_scene,
};
use crate::scenes::game::build_game_scene;

// ── Config ────────────────────────────────────────────────────────────────────

/// Reach used by the auto-player when deciding whether a hook is grabbable.
const AI_REACH: f32 = ROPE_LEN_MAX;
/// How far the bot looks for shelter during a flare. Must exceed
/// `FLARE_SHELTER_SEARCH_AHEAD` so the bot can see every shelter the flare
/// system considered reachable when it decided to fire.
const AI_SHELTER_SCAN: f32 = 8000.0;
/// Release the rope once we're moving right this fast (px/tick).
const AI_RELEASE_VX: f32 = 14.0;
/// ...and once total speed clears this, so we don't release while nearly still.
const AI_RELEASE_SPEED: f32 = 26.0;
/// If hooked longer than this without meeting the release condition, let go.
const AI_FORCE_RELEASE_TICKS: u32 = 100;
/// Frames to play normally before forcing a fall in `--fall-test` mode.
const FALL_TEST_FRAME: u64 = 200;

#[derive(Debug, Clone)]
pub struct EpisodeReport {
    pub frames: u64,
    pub max_dist: f32,
    pub final_dist: f32,
    pub max_speed: f32,
    pub coins: i32,
    pub hooks_grabbed: u64,
    pub death_scene: Option<String>,
    pub space_entered: bool,
    pub zone: usize,
    pub boss_entered: bool,
    pub boss_killed: bool,
    pub boss_hp_seen: i32,
    pub weakpoint_hit: bool,
    pub hearts_lost: i64,
    pub hearts_end: i32,
    pub panicked: Option<String>,
    /// Frames spent unhooked, inside the reachable band, with NO grab node
    /// within `AI_REACH`.
    ///
    /// This is the "impossible stretch" instrument, and it is deliberately
    /// independent of how well the bot plays: a starved frame means the world
    /// offered the player nothing to reach for, whatever they did. It does not
    /// have to be zero — the player is airborne between nodes by design — but a
    /// run of consecutive starved frames is a hole in the level.
    pub starved_frames: u64,
    pub airborne_frames: u64,
    /// Longest unbroken run of starved frames in the episode.
    pub worst_starve_streak: u64,
    /// Solar-flare telemetry. `flares_fired` counting up while
    /// `flares_without_shelter` stays at zero is the check that matters: a
    /// flare must never begin its telegraph with no reachable shielded node.
    pub flares_fired: i32,
    pub flare_hearts_lost: i32,
    pub flare_saves: i32,
    pub flares_without_shelter: i32,
    /// Largest gap ever seen between the chain frontier and the player. Healthy
    /// steady state hovers just above GEN_AHEAD; a large value means world
    /// generation was switched off and the player was crossing empty world.
    pub worst_frontier_overshoot: f32,
    /// Times the frontier guard had to repair `rightmost_x`. Must stay 0.
    pub frontier_repairs: i32,
    /// How many of each hazard the episode ever saw alive. The introduction
    /// schedule is only real if the world agrees with it, so the harness counts
    /// objects rather than trusting the curve.
    pub census: HazardCensus,
}

/// Peak simultaneous count of each object type seen during an episode.
#[derive(Debug, Default, Clone, Copy)]
pub struct HazardCensus {
    pub hooks: usize,
    pub pads: usize,
    pub spinners: usize,
    pub gwells: usize,
    pub turrets: usize,
    pub high_asteroids: usize,
    pub drift_asteroids: usize,
}

impl HazardCensus {
    fn absorb(&mut self, o: &Self) {
        self.hooks = self.hooks.max(o.hooks);
        self.pads = self.pads.max(o.pads);
        self.spinners = self.spinners.max(o.spinners);
        self.gwells = self.gwells.max(o.gwells);
        self.turrets = self.turrets.max(o.turrets);
        self.high_asteroids = self.high_asteroids.max(o.high_asteroids);
        self.drift_asteroids = self.drift_asteroids.max(o.drift_asteroids);
    }
}

#[derive(Debug, Default)]
pub struct AggregateReport {
    pub episodes: u64,
    pub panics: u64,
    pub total_frames: u64,
    pub avg_dist: f32,
    pub best_dist: f32,
    pub avg_max_speed: f32,
    pub total_hooks_grabbed: u64,
    pub total_coins: i32,
    pub deaths: u64,
    pub space_entries: u64,
    pub max_zone: usize,
    pub boss_entries: u64,
    pub boss_kills: u64,
    pub total_hearts_lost: i64,
    pub final_hearts_sum: i32,
    pub weakpoint_hits: u64,
    pub death_scene_histogram: std::collections::HashMap<String, u64>,
    pub starved_frames: u64,
    pub airborne_frames: u64,
    pub worst_starve_streak: u64,
    pub flares_fired: i32,
    pub flare_hearts_lost: i32,
    pub flare_saves: i32,
    pub flares_without_shelter: i32,
    pub worst_frontier_overshoot: f32,
    pub frontier_repairs: i32,
    pub census: HazardCensus,
}

// ── Canvas factory (mirrors App::new but boots straight into the game) ────────

/// `start_minute` must be applied BEFORE `load_scene("game")`: the game scene's
/// `on_enter` builds `State` immediately, so a var set after this call arrives
/// a whole run too late — which is exactly how the first version of the
/// difficulty sampler silently reported an empty schedule at every minute.
fn build_canvas(ctx: &mut prism::Context, start_minute: f32) -> Canvas {
    let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
    if start_minute > 0.0 {
        canvas.set_var(
            "debug_start_distance",
            Value::F32(start_minute * crate::difficulty::DIFFICULTY_PX_PER_MINUTE),
        );
    }
    canvas.add_scene(build_tutorial_scene(ctx));
    canvas.add_scene(build_menu_scene(ctx));
    canvas.add_scene(build_game_scene(ctx));
    canvas.add_scene(build_gameover_scene(ctx));
    canvas.add_scene(build_gameover_sun_scene(ctx));
    canvas.add_scene(build_gameover_oxygen_scene(ctx));
    canvas.add_scene(build_menu_settings_scene(ctx));
    canvas.add_scene(build_achievements_scene(ctx));
    canvas.add_scene(build_stats_scene(ctx));
    canvas.add_scene(build_daily_reward_scene(ctx));
    canvas.load_scene("game");
    canvas
}

// ── Observation helpers ───────────────────────────────────────────────────────

struct Obs {
    px: f32,
    py: f32,
    vx: f32,
    vy: f32,
    hooked: bool,
    /// (x, y, is_shielded) for every grab node within reach.
    hooks: Vec<(f32, f32, bool)>,
    /// Shielded nodes within a WIDE radius — well beyond grab reach. Routing to
    /// shelter is a multi-hop problem, so the bot needs to see the destination
    /// long before it can grab it.
    shelters: Vec<(f32, f32)>,
}

fn get_i32_or(c: &Canvas, name: &str, default: i32) -> i32 {
    match c.get_var(name) {
        Some(Value::I32(v)) => v,
        _ => default,
    }
}

fn observe(c: &Canvas) -> Option<Obs> {
    let (px, py, vx, vy) = {
        let p = c.get_game_object("player")?;
        (
            p.position.0 + p.size.0 * 0.5,
            p.position.1 + p.size.1 * 0.5,
            p.momentum.0,
            p.momentum.1,
        )
    };

    let hooked = c.get_game_object("rope").map(|o| o.visible).unwrap_or(false);

    // Collect hook centres within AI_REACH of the player.
    let hooks = if let Some(p) = c.get_game_object("player") {
        c.objects_in_radius(p, AI_REACH)
            .into_iter()
            .filter(|o| o.tags.iter().any(|t| t == "hook"))
            .map(|o| (
                o.position.0 + o.size.0 * 0.5,
                o.position.1 + o.size.1 * 0.5,
                o.tags.iter().any(|t| t == crate::constants::SHIELD_HOOK_TAG),
            ))
            .filter(|(hx, hy, _)| {
                let dx = *hx - px;
                let dy = *hy - py;
                dx * dx + dy * dy <= AI_REACH * AI_REACH
            })
            .collect()
    } else {
        Vec::new()
    };

    let shelters = if let Some(p) = c.get_game_object("player") {
        c.objects_in_radius(p, AI_SHELTER_SCAN)
            .into_iter()
            .filter(|o| o.tags.iter().any(|t| t == crate::constants::SHIELD_HOOK_TAG))
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5))
            .collect()
    } else {
        Vec::new()
    };

    Some(Obs { px, py, vx, vy, hooked, hooks, shelters })
}

// ── Auto-swing policy ─────────────────────────────────────────────────────────
// Return (hold-space, target-hook). When free, steer toward the best ahead hook
// (via mouse-targeted grab) so the bot chains forward instead of oscillating.
// When hooked, release once we're moving forward fast enough (or after a stall).

/// Nearest known shelter, at any distance.
fn nearest_shelter_point(o: &Obs) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    for &(hx, hy) in &o.shelters {
        let dx = hx - o.px;
        let dy = hy - o.py;
        let d2 = dx * dx + dy * dy;
        if best.map_or(true, |(bd2, _, _)| d2 < bd2) {
            best = Some((d2, hx, hy));
        }
    }
    best.map(|(_, hx, hy)| (hx, hy))
}

/// Nearest shielded node in reach, if any.
fn pick_shelter(o: &Obs) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    for &(hx, hy, shielded) in &o.hooks {
        if !shielded {
            continue;
        }
        let dx = hx - o.px;
        let dy = hy - o.py;
        let d2 = dx * dx + dy * dy;
        if best.map_or(true, |(bd2, _, _)| d2 < bd2) {
            best = Some((d2, hx, hy));
        }
    }
    best.map(|(_, hx, hy)| (hx, hy))
}

fn decide(
    o: &Obs,
    hooked_ticks: u32,
    force_fall: bool,
    frames: u64,
    buffed: bool,
    boss_center: Option<(f32, f32)>,
    flare_threat: bool,
    sheltered: bool,
) -> (bool, Option<(f32, f32)>) {
    if force_fall && frames >= FALL_TEST_FRAME {
        // Force a fall so we exercise the heart-loss / respawn path.
        return (false, None);
    }
    // Under flare threat, shelter beats progress: hold a shielded node rather
    // than releasing from it, and steer to one when free. Without this the bot
    // cannot exercise the save path at all, and `flare_saves` stays at zero
    // whether the mechanic works or not.
    if flare_threat {
        let shelter_in_reach = pick_shelter(o);
        if o.hooked {
            if sheltered {
                // Already safe — hold on for the rest of the window.
                return (true, None);
            }
            // Only let go once shelter is actually grabbable. Releasing on the
            // first threatened frame just drops the bot into freefall and the
            // shelter is out of reach again by the time it can act.
            if shelter_in_reach.is_some() {
                return (false, None);
            }
            // Otherwise keep swinging normally, which is what carries us toward
            // the shelter in the first place.
            let speed = (o.vx * o.vx + o.vy * o.vy).sqrt();
            let release = (o.vx > AI_RELEASE_VX && speed > AI_RELEASE_SPEED)
                || hooked_ticks >= AI_FORCE_RELEASE_TICKS;
            return (!release, None);
        }
        if let Some(target) = shelter_in_reach {
            return (true, Some(target));
        }
        // Shelter known but out of grab range: hop toward it by taking the
        // in-reach node closest to it. Same greedy routing the boss targeting
        // uses, pointed at a different destination.
        if let Some(dest) = nearest_shelter_point(o) {
            if let Some(step) = pick_nearest_to(o, dest) {
                return (true, Some(step));
            }
        }
    }
    if o.hooked {
        let speed = (o.vx * o.vx + o.vy * o.vy).sqrt();
        let release = (o.vx > AI_RELEASE_VX && speed > AI_RELEASE_SPEED)
            || hooked_ticks >= AI_FORCE_RELEASE_TICKS;
        return (!release, None);
    }
    // Boss attack: while buffed and the boss is visible, swing at the hook
    // nearest the boss so the arc passes through a weakpoint.
    if buffed {
        if let Some(bc) = boss_center {
            if let Some((hx, hy)) = pick_nearest_to(o, bc) {
                return (true, Some((hx, hy)));
            }
        }
    }
    match pick_best_target(o) {
        Some((hx, hy)) => (true, Some((hx, hy))),
        None => (false, None),
    }
}

/// Pick the hook (within reach) nearest to a world point.
fn pick_nearest_to(o: &Obs, point: (f32, f32)) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None; // (dist2, hx, hy)
    for &(hx, hy, _) in &o.hooks {
        let dx = hx - point.0;
        let dy = hy - point.1;
        let d2 = dx * dx + dy * dy;
        if best.map_or(true, |(bd2, _, _)| d2 < bd2) {
            best = Some((d2, hx, hy));
        }
    }
    best.map(|(_, hx, hy)| (hx, hy))
}

/// Pick the hook that best advances the run: within reach, minimising distance
/// while preferring forward progress (ahead of the player scores lower).
fn pick_best_target(o: &Obs) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None; // (score, hx, hy)
    for &(hx, hy, _) in &o.hooks {
        let dx = hx - o.px;
        let dy = hy - o.py;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > AI_REACH {
            continue;
        }
        let forward = dx.max(0.0);
        let score = dist - forward * 1.5;
        if best.map_or(true, |(s, _, _)| score < s) {
            best = Some((score, hx, hy));
        }
    }
    best.map(|(_, hx, hy)| (hx, hy))
}

// ── Input injection ───────────────────────────────────────────────────────────

fn send_key(ctx: &mut prism::Context, c: &mut Canvas, sized: &SizedTree, state: KeyboardState) {
    let evt = KeyboardEvent {
        key: Key::Named(NamedKey::Space),
        state,
        modifiers: Modifiers::none(),
    };
    OnEvent::on_event(c, ctx, sized, Box::new(evt) as Box<dyn Event>);
}

fn zone_for_distance(d: f32) -> usize {
    crate::difficulty::zone_index_for_distance(d)
}

// ── Episode runner ────────────────────────────────────────────────────────────

fn run_episode(max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool, weakpoint_check: bool, flare_test: bool, shelter_check: bool, start_minute: f32) -> EpisodeReport {
    let (mut ctx, _recv) = prism::Context::new();
    let mut canvas = build_canvas(&mut ctx, start_minute);
    let sized = SizedTree::default();

    if boss_mode {
        canvas.set_var("boss_mode_active", true);
    }
    if boss_warp {
        // Force boss entry immediately; the boss entry routine warps the player
        // into the arena (with tether nodes) regardless of player position.
        canvas.set_var("force_boss_warp", true);
    }
    if weakpoint_check {
        // The boss battle starts when the player tethers a node. The auto-player
        // holds space continuously so it never issues a fresh grab during the
        // entry stasis — bypass the tether so the validation can proceed.
        canvas.set_var("debug_boss_stasis_down", true);
    }
    if flare_test || shelter_check {
        // Fire a flare every ~4 s so one episode exercises many, instead of the
        // at-most-one a shipped 90 s interval would produce.
        canvas.set_var("debug_flare_interval", 240i32);
    }

    // Use mouse-targeted grabbing so the bot can steer toward ahead hooks
    // instead of always grabbing the nearest (which causes oscillation).
    canvas.set_var("grab_from_mouse", true);

    // Press space to resume from "HOLD SPACE TO BEGIN" and stay held so the
    // first gameplay frame is a fresh rising edge → immediate first grab.
    send_key(&mut ctx, &mut canvas, &sized, KeyboardState::Pressed);
    let mut space_held = true;

    let mut frames: u64 = 0;
    let mut max_dist = 0.0f32;
    let mut max_speed = 0.0f32;
    let mut hooks_grabbed: u64 = 0;
    let mut prev_rope = false;
    let mut hooked_ticks: u32 = 0;
    let mut min_py = f32::MAX;
    let mut coins = 0i32;
    let mut boss_entered = false;
    let mut boss_killed = false;
    let mut boss_hp_seen = crate::constants::BOSS_MAX_HP;
    let mut weakpoint_hit = false;
    let mut weakpoint_checked = false;
    let mut weakpoint_check_ticks: u32 = 0;
    let mut death_scene: Option<String> = None;
    let mut starved_frames: u64 = 0;
    let mut airborne_frames: u64 = 0;
    let mut starve_streak: u64 = 0;
    let mut worst_starve_streak: u64 = 0;
    let mut flares_without_shelter: i32 = 0;
    let mut prev_flare_count: i32 = 0;
    let mut census = HazardCensus::default();
    let mut worst_overshoot = 0.0f32;

    while frames < max_frames {
        // Observe (borrows canvas immutably, then released).
        let Some(o) = observe(&canvas) else {
            break;
        };
        if o.hooked {
            hooked_ticks += 1;
            starve_streak = 0;
        } else {
            hooked_ticks = 0;
            // Only frames where the player is still INSIDE the reachable band
            // count. Below it there is nothing to grab by construction, so
            // counting those frames measures the length of a death animation
            // rather than a hole in the level — which is what this is for.
            // The space zone has its own node band far above the normal one, so
            // its frames are neither in-band nor a hole in the level — counting
            // them made a run that reached space look twice as starved as one
            // that did not.
            let in_space = matches!(canvas.get_var("in_space_mode"), Some(Value::Bool(true)));
            let in_band = !in_space
                && o.py >= HOOK_Y_MIN - ROPE_LEN_MAX
                && o.py <= HOOK_Y_MAX + ROPE_LEN_MAX;
            if in_band {
                airborne_frames += 1;
                if o.hooks.is_empty() {
                    starved_frames += 1;
                    starve_streak += 1;
                    worst_starve_streak = worst_starve_streak.max(starve_streak);
                } else {
                    starve_streak = 0;
                }
            } else {
                starve_streak = 0;
            }
        }

        // Shelter check: during a telegraph, move the player onto the nearest
        // shielded node so the real grab path and the real shelter rule run
        // deterministically. The greedy bot cannot route several hops to a
        // specific node, so without this the save path is never exercised and
        // `flare_saves` reads zero whether the mechanic works or not.
        if shelter_check
            && matches!(canvas.get_var("flare_warning"), Some(Value::Bool(true)))
        {
            if let Some((sx, sy)) = o.shelters.first().copied() {
                if let Some(p) = canvas.get_game_object_mut("player") {
                    p.position = (sx - crate::constants::PLAYER_R,
                                  sy - crate::constants::PLAYER_R * 3.0);
                    p.momentum = (0.0, 0.0);
                }
            }
        }

        // Flare audit: the frame a new flare's telegraph starts, confirm a
        // shielded node was actually within reach. This is the invariant the
        // whole mechanic rests on, so it is checked from outside the system
        // that is supposed to maintain it.
        {
            let count = get_i32_or(&canvas, "flares_fired", 0);
            if count > prev_flare_count {
                prev_flare_count = count;
                let shelter_near = canvas
                    .get_game_object("player")
                    .map(|p| {
                        let px = p.position.0 + p.size.0 * 0.5;
                        canvas
                            .objects_in_radius(p, crate::constants::FLARE_SHELTER_SEARCH_AHEAD)
                            .into_iter()
                            .any(|o| {
                                o.tags.iter().any(|t| t == crate::constants::SHIELD_HOOK_TAG)
                                    && (o.position.0 + o.size.0 * 0.5) - px
                                        <= crate::constants::FLARE_SHELTER_SEARCH_AHEAD
                            })
                    })
                    .unwrap_or(false);
                if !shelter_near {
                    flares_without_shelter += 1;
                }
            }
        }

        // Census: count what is actually alive this frame. Cheap enough at
        // headless speeds, and it is the only way to prove the introduction
        // schedule reaches the world rather than just the curve.
        {
            let mut now = HazardCensus::default();
            // No general enumeration API on Canvas, and a radius sweep from the
            // player is the more meaningful measure anyway: it counts what is
            // actually near enough to matter rather than everything pooled.
            let nearby: Vec<&quartz::GameObject> = match canvas.get_game_object("player") {
                Some(p) => canvas.objects_in_radius(p, crate::constants::GEN_AHEAD),
                None => Vec::new(),
            };
            for obj in nearby {
                if !obj.visible {
                    continue;
                }
                let is_drift = obj.tags.iter().any(|t| t == crate::constants::ASTEROID_DRIFT_TAG);
                if obj.id.starts_with("space_asteroid") {
                    if is_drift { now.drift_asteroids += 1; } else { now.high_asteroids += 1; }
                } else if obj.tags.iter().any(|t| t == "hook") {
                    now.hooks += 1;
                } else if obj.id.starts_with("pad_") && !obj.id.ends_with("_thruster") {
                    now.pads += 1;
                } else if obj.id.starts_with("spinner") {
                    now.spinners += 1;
                } else if obj.id.starts_with("gwell") {
                    now.gwells += 1;
                } else if obj.id.starts_with("turret") {
                    now.turrets += 1;
                }
            }
            census.absorb(&now);
        }

        // Frontier health: how far ahead of the player the chain claims to
        // extend. Steady state sits just above GEN_AHEAD; a spike means
        // generation stalled and the player is crossing empty world.
        {
            let frontier = match canvas.get_var("hook_frontier_ahead") {
                Some(Value::F32(v)) => v,
                _ => 0.0,
            };
            if frontier > worst_overshoot {
                worst_overshoot = frontier;
            }
        }

        let buffed = get_i32_or(&canvas, "player_buff", 0) > 0;
        let boss_visible = canvas.get_game_object("boss").map(|b| b.visible).unwrap_or(false);
        let boss_center = canvas.get_game_object("boss").map(|b| {
            (b.position.0 + b.size.0 * 0.5, b.position.1 + b.size.1 * 0.5)
        });

        let (hold, target) = if weakpoint_check && !weakpoint_checked && boss_visible {
            // Deterministic weakpoint validation: force a buff, drop the boss
            // forcefield (as if all generators were destroyed), and pin the player
            // on a weakpoint each frame so the contact logic fires.
            canvas.set_var("debug_boss_forcefield_down", true);
            let bc = boss_center.unwrap();
            let (ox, oy) = crate::constants::BOSS_WEAKPOINT_OFFSETS[0];
            if let Some(p) = canvas.get_game_object_mut("player") {
                p.position.0 = bc.0 + ox - PLAYER_R;
                p.position.1 = bc.1 + oy - PLAYER_R;
                p.momentum = (0.0, 0.0);
                p.gravity = 0.0;
            }
            canvas.set_var("debug_force_buff", true);
            weakpoint_check_ticks += 1;
            let bh = get_i32_or(&canvas, "boss_hp", crate::constants::BOSS_MAX_HP);
            if bh < crate::constants::BOSS_MAX_HP {
                weakpoint_hit = true;
                weakpoint_checked = true;
            } else if weakpoint_check_ticks > 10 {
                weakpoint_checked = true;
            }
            (false, None)
        } else {
            let flare_threat = matches!(canvas.get_var("flare_warning"), Some(Value::Bool(true)))
                || matches!(canvas.get_var("flare_active"), Some(Value::Bool(true)));
            let sheltered = matches!(canvas.get_var("player_sheltered"), Some(Value::Bool(true)));

            decide(&o, hooked_ticks, force_fall, frames, buffed, boss_center, flare_threat, sheltered)
        };

        if let Some((tx, ty)) = target {
            canvas.set_var("mouse_grab_x", Value::F32(tx));
            canvas.set_var("mouse_grab_y", Value::F32(ty));
        }
        if hold != space_held {
            let state = if hold { KeyboardState::Pressed } else { KeyboardState::Released };
            send_key(&mut ctx, &mut canvas, &sized, state);
            space_held = hold;
        }

        // Run one engine frame.
        OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);

        frames += 1;

        // Detect death BEFORE observing — the player object is removed on a
        // scene switch, so observe() would return None and mask the death.
        if canvas.is_scene("gameover_sun") {
            death_scene = Some("sun".to_string());
            break;
        } else if canvas.is_scene("gameover_oxygen") {
            death_scene = Some("oxygen".to_string());
            break;
        } else if canvas.is_scene("gameover") {
            death_scene = Some("fall".to_string());
            break;
        }

        // Post-frame observation.
        let Some(o) = observe(&canvas) else { break; };
        // Arena X is not progress — the player is warped to a region two
        // million pixels out, so counting it reported every boss run as a
        // record-breaking distance.
        let in_arena = matches!(canvas.get_var("boss_active"), Some(Value::Bool(true)));
        if !in_arena {
            let dist = (o.px - SPAWN_X).max(0.0);
            if dist > max_dist {
                max_dist = dist;
            }
        }
        let speed = (o.vx * o.vx + o.vy * o.vy).sqrt();
        if speed > max_speed {
            max_speed = speed;
        }
        if o.py < min_py {
            min_py = o.py;
        }
        // Count hook grabs as rope visible false→true transitions.
        if o.hooked && !prev_rope {
            hooks_grabbed += 1;
        }
        prev_rope = o.hooked;

        // Boss progression detection.
        if canvas.get_game_object("boss").map(|b| b.visible).unwrap_or(false) {
            boss_entered = true;
        }
        if matches!(canvas.get_var("boss_defeated_this_fight"), Some(Value::Bool(true))) {
            boss_killed = true;
        }
        let bh = get_i32_or(&canvas, "boss_hp", crate::constants::BOSS_MAX_HP);
        if bh < boss_hp_seen {
            boss_hp_seen = bh;
        }

        coins = get_i32_or(&canvas, TOTAL_COINS_COLLECTED_VAR, 0);
    }

    let final_o = observe(&canvas);
    let final_dist = if matches!(canvas.get_var("boss_active"), Some(Value::Bool(true))) {
        max_dist
    } else {
        final_o.map(|o| (o.px - SPAWN_X).max(0.0)).unwrap_or(max_dist)
    };
    let hearts_lost = get_i32_or(&canvas, "heart_losses", 0) as i64;
    let hearts_end = get_i32_or(&canvas, "hearts", MAX_HEARTS);

    EpisodeReport {
        frames,
        max_dist,
        final_dist,
        max_speed,
        coins,
        hooks_grabbed,
        death_scene,
        space_entered: min_py < -(VH * 1.1),
        zone: zone_for_distance(max_dist),
        boss_entered,
        boss_killed,
        boss_hp_seen,
        weakpoint_hit,
        hearts_lost,
        hearts_end,
        panicked: None,
        starved_frames,
        airborne_frames,
        worst_starve_streak,
        flares_fired: get_i32_or(&canvas, "flares_fired", 0),
        flare_hearts_lost: get_i32_or(&canvas, "flare_hearts_lost", 0),
        flare_saves: get_i32_or(&canvas, "flare_saves", 0),
        flares_without_shelter,
        worst_frontier_overshoot: worst_overshoot,
        frontier_repairs: get_i32_or(&canvas, "frontier_repairs", 0),
        census,
    }
}

/// Run several episodes (each boots a fresh canvas) and aggregate.
pub fn run(episodes: u64, max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool, weakpoint_check: bool, flare_test: bool, shelter_check: bool, start_minute: f32) -> AggregateReport {
    let mut agg = AggregateReport::default();
    let mut episode_idx = 0u64;
    while episode_idx < episodes {
        let ep = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_episode(max_frames, boss_mode, force_fall, boss_warp, weakpoint_check, flare_test, shelter_check, start_minute)
        }))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            EpisodeReport {
                frames: 0,
                max_dist: 0.0,
                final_dist: 0.0,
                max_speed: 0.0,
                coins: 0,
                hooks_grabbed: 0,
                death_scene: Some("panic".to_string()),
                space_entered: false,
                zone: 0,
                boss_entered: false,
                boss_killed: false,
                boss_hp_seen: crate::constants::BOSS_MAX_HP,
                weakpoint_hit: false,
                hearts_lost: 0,
                hearts_end: 0,
                panicked: Some(msg),
                starved_frames: 0,
                airborne_frames: 0,
                worst_starve_streak: 0,
                flares_fired: 0,
                flare_hearts_lost: 0,
                flare_saves: 0,
                flares_without_shelter: 0,
                worst_frontier_overshoot: 0.0,
                frontier_repairs: 0,
                census: HazardCensus::default(),
            }
        });

        agg.episodes += 1;
        agg.total_frames += ep.frames;
        if ep.panicked.is_some() {
            agg.panics += 1;
        }
        agg.avg_dist += ep.max_dist;
        agg.best_dist = agg.best_dist.max(ep.max_dist);
        agg.avg_max_speed += ep.max_speed;
        agg.total_hooks_grabbed += ep.hooks_grabbed;
        agg.total_coins += ep.coins;
        if ep.death_scene.is_some() {
            agg.deaths += 1;
            let key = ep.death_scene.clone().unwrap();
            *agg.death_scene_histogram.entry(key).or_insert(0) += 1;
        }
        if ep.space_entered {
            agg.space_entries += 1;
        }
        if ep.boss_entered {
            agg.boss_entries += 1;
        }
        if ep.boss_killed {
            agg.boss_kills += 1;
        }
        agg.total_hearts_lost += ep.hearts_lost;
        agg.final_hearts_sum += ep.hearts_end;
        if ep.weakpoint_hit {
            agg.weakpoint_hits += 1;
        }
        agg.max_zone = agg.max_zone.max(ep.zone);
        agg.starved_frames += ep.starved_frames;
        agg.airborne_frames += ep.airborne_frames;
        agg.worst_starve_streak = agg.worst_starve_streak.max(ep.worst_starve_streak);
        agg.flares_fired += ep.flares_fired;
        agg.flare_hearts_lost += ep.flare_hearts_lost;
        agg.flare_saves += ep.flare_saves;
        agg.flares_without_shelter += ep.flares_without_shelter;
        agg.census.absorb(&ep.census);
        agg.worst_frontier_overshoot = agg.worst_frontier_overshoot.max(ep.worst_frontier_overshoot);
        agg.frontier_repairs += ep.frontier_repairs;

        // Progress line.
        println!(
            "ep {}  frames={}  dist={:.0}  speed={:.1}  coins={}  grabs={}  death={:?}  zone={}  space={}  bossIn={}  bossKill={}  bossHP={}  weakHit={}  heartsLost={}  heartsEnd={}  starved={}/{}  worstStreak={}",
            episode_idx,
            ep.frames,
            ep.max_dist,
            ep.max_speed,
            ep.coins,
            ep.hooks_grabbed,
            ep.death_scene,
            ep.zone,
            ep.space_entered,
            ep.boss_entered,
            ep.boss_killed,
            ep.boss_hp_seen,
            ep.weakpoint_hit,
            ep.hearts_lost,
            ep.hearts_end,
            ep.starved_frames,
            ep.airborne_frames,
            ep.worst_starve_streak,
        );

        episode_idx += 1;
    }

    if agg.episodes > 0 {
        agg.avg_dist /= agg.episodes as f32;
        agg.avg_max_speed /= agg.episodes as f32;
    }
    agg
}
