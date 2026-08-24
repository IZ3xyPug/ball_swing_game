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
    pub hearts_lost: i64,
    pub hearts_end: i32,
    pub panicked: Option<String>,
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
    pub death_scene_histogram: std::collections::HashMap<String, u64>,
}

// ── Canvas factory (mirrors App::new but boots straight into the game) ────────

fn build_canvas(ctx: &mut prism::Context) -> Canvas {
    let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
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
    hooks: Vec<(f32, f32)>,
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
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5))
            .filter(|(hx, hy)| {
                let dx = *hx - px;
                let dy = *hy - py;
                dx * dx + dy * dy <= AI_REACH * AI_REACH
            })
            .collect()
    } else {
        Vec::new()
    };

    Some(Obs { px, py, vx, vy, hooked, hooks })
}

// ── Auto-swing policy ─────────────────────────────────────────────────────────
// Return (hold-space, target-hook). When free, steer toward the best ahead hook
// (via mouse-targeted grab) so the bot chains forward instead of oscillating.
// When hooked, release once we're moving forward fast enough (or after a stall).

fn decide(
    o: &Obs,
    hooked_ticks: u32,
    force_fall: bool,
    frames: u64,
    buffed: bool,
    boss_center: Option<(f32, f32)>,
) -> (bool, Option<(f32, f32)>) {
    if force_fall && frames >= FALL_TEST_FRAME {
        // Force a fall so we exercise the heart-loss / respawn path.
        return (false, None);
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
    for &(hx, hy) in &o.hooks {
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
    for &(hx, hy) in &o.hooks {
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
    ((d / ZONE_DISTANCE_STEP) as usize).min(2)
}

// ── Episode runner ────────────────────────────────────────────────────────────

fn run_episode(max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool) -> EpisodeReport {
    let (mut ctx, _recv) = prism::Context::new();
    let mut canvas = build_canvas(&mut ctx);
    let sized = SizedTree::default();

    if boss_mode {
        canvas.set_var("boss_mode_active", true);
    }
    if boss_warp {
        // Force boss entry immediately; the boss entry routine warps the player
        // into the arena (with tether nodes) regardless of player position.
        canvas.set_var("force_boss_warp", true);
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
    let mut death_scene: Option<String> = None;

    while frames < max_frames {
        // Observe (borrows canvas immutably, then released).
        let Some(o) = observe(&canvas) else {
            break;
        };
        if o.hooked {
            hooked_ticks += 1;
        } else {
            hooked_ticks = 0;
        }

        let buffed = get_i32_or(&canvas, "player_buff", 0) > 0;
        let boss_center = canvas.get_game_object("boss").map(|b| {
            (b.position.0 + b.size.0 * 0.5, b.position.1 + b.size.1 * 0.5)
        });
        let (hold, target) = decide(&o, hooked_ticks, force_fall, frames, buffed, boss_center);
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
        let dist = (o.px - SPAWN_X).max(0.0);
        if dist > max_dist {
            max_dist = dist;
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
        if canvas.get_bool("boss_mode_cleared") {
            boss_killed = true;
        }
        let bh = get_i32_or(&canvas, "boss_hp", crate::constants::BOSS_MAX_HP);
        if bh < boss_hp_seen {
            boss_hp_seen = bh;
        }

        coins = get_i32_or(&canvas, TOTAL_COINS_COLLECTED_VAR, 0);
    }

    let final_o = observe(&canvas);
    let final_dist = final_o.map(|o| (o.px - SPAWN_X).max(0.0)).unwrap_or(max_dist);
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
        hearts_lost,
        hearts_end,
        panicked: None,
    }
}

/// Run several episodes (each boots a fresh canvas) and aggregate.
pub fn run(episodes: u64, max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool) -> AggregateReport {
    let mut agg = AggregateReport::default();
    let mut episode_idx = 0u64;
    while episode_idx < episodes {
        let ep = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_episode(max_frames, boss_mode, force_fall, boss_warp)
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
                hearts_lost: 0,
                hearts_end: 0,
                panicked: Some(msg),
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
        agg.max_zone = agg.max_zone.max(ep.zone);

        // Progress line.
        println!(
            "ep {}  frames={}  dist={:.0}  speed={:.1}  coins={}  grabs={}  death={:?}  zone={}  space={}  bossIn={}  bossKill={}  bossHP={}  heartsLost={}  heartsEnd={}",
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
            ep.hearts_lost,
            ep.hearts_end,
        );

        episode_idx += 1;
    }

    if agg.episodes > 0 {
        agg.avg_dist /= agg.episodes as f32;
        agg.avg_max_speed /= agg.episodes as f32;
    }
    agg
}
