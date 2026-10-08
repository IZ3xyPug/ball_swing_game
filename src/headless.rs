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

/// Pin every roster slot to one boss, by its `BossKind::slug`.
///
/// Exposed here rather than by opening `constants` to the world: the binary is
/// the only caller, and the module boundary is what keeps the tuning tables
/// from leaking into anything that is not the game.
///
/// Every slot, not just the first — a short override falls through to the
/// shipped roster past its end, so a one-entry list fights this boss once and
/// then goes back to the normal order.
pub fn pin_boss(name: &str) -> Result<(), Vec<&'static str>> {
    match boss_kind_by_name(name) {
        Some(kind) => {
            set_boss_order_override(Some(vec![kind; crate::mode::BOSS_ROSTER_SIZE as usize]));
            Ok(())
        }
        None => Err(boss_kind_names()),
    }
}

/// The names `pin_boss` accepts, for help text.
pub fn boss_slugs() -> Vec<&'static str> {
    boss_kind_names()
}
use crate::achievements::TOTAL_COINS_COLLECTED_VAR;
use crate::menu::{
    build_tutorial_scene, build_menu_scene, build_gameover_oxygen_scene, build_gameover_scene,
    build_gameover_sun_scene, build_menu_settings_scene, build_achievements_scene,
    build_stats_scene, build_daily_reward_scene, build_profile_scene,
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
    /// Frames on which `player_buff > 0` while `buff_timer == 0`.
    ///
    /// The buff is read as `player_buff > 0` by every boss, the solar shield
    /// and the weakpoint checks, but its lifetime is `buff_timer`. If the two
    /// ever disagree the player is mechanically buffed with no buff running —
    /// which is how the Colossus stopped dealing damage. Must be 0.
    pub buff_desync_frames: u64,
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
    /// Distance (px, along x) from the player to the nearest shelter at each
    /// flare's start, summed and worst: how far the player has to go, which
    /// the pass/fail count above cannot say.
    pub shelter_dx_sum: f32,
    pub shelter_dx_worst: f32,
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
    /// Total frames where the buff's two fields disagreed. Must be 0.
    pub buff_desync_frames: u64,
    pub airborne_frames: u64,
    pub worst_starve_streak: u64,
    pub flares_fired: i32,
    pub flare_hearts_lost: i32,
    pub flare_saves: i32,
    pub flares_without_shelter: i32,
    /// Distance (px, along x) from the player to the nearest shelter at each
    /// flare's start, summed and worst: how far the player has to go, which
    /// the pass/fail count above cannot say.
    pub shelter_dx_sum: f32,
    pub shelter_dx_worst: f32,
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
    // Point saves at a temp dir so headless runs (which award meta / write
    // profiles) never clobber the real save slots.
    let tmp = std::env::temp_dir().join("ball_swing_headless_saves");
    crate::profile::set_saves_dir(tmp.to_str().unwrap_or("saves"));
    let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
    // Same switch the game uses, so a headless run can be profiled by stage.
    canvas.set_tick_profiling(std::env::var("QUARTZ_TICK_PROFILE").is_ok_and(|v| v != "0"));
    if start_minute > 0.0 {
        canvas.set_var(
            "debug_start_distance",
            Value::F32(start_minute * crate::difficulty::DIFFICULTY_PX_PER_MINUTE),
        );
    }
    // Looks for captures: HEADLESS_CHAR / HEADLESS_ROPE_STYLE /
    // HEADLESS_ROPE_COLOR (indices), applied before the game scene builds.
    for (env, var) in [
        ("HEADLESS_CHAR", "player_char_selected"),
        ("HEADLESS_ROPE_STYLE", "player_rope_style_selected"),
        ("HEADLESS_ROPE_COLOR", "player_rope_selected"),
    ] {
        if let Some(v) = std::env::var(env).ok().and_then(|v| v.parse::<i32>().ok()) {
            canvas.set_var(var, v);
        }
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
    canvas.add_scene(build_profile_scene(ctx));
    crate::scenes::game::events::register_mouse_handlers(&mut canvas);
    crate::scenes::game::death_screen::register_handlers(&mut canvas);
    crate::transition::register_hold_key(&mut canvas);
    // `HEADLESS_PERM=<id>:<rank>,...`: permanent upgrade ranks for this run
    // (filming the shop's upgrade demos: the same run without, then with).
    if let Ok(spec) = std::env::var("HEADLESS_PERM") {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        for part in spec.split(',') {
            let Some((id, r)) = part.split_once(':') else { continue };
            let r: u32 = r.parse().unwrap_or(0);
            if id == "heart" { p.permanent_extra_hearts = r; continue; }
            p.perm_levels.retain(|(k, _)| k != id);
            p.perm_levels.push((id.to_string(), r));
        }
    }
    // `HEADLESS_ABILITY=<id>:<rank>`: equip an ability for this run.
    if let Some((id, rank)) = std::env::var("HEADLESS_ABILITY").ok().as_deref().and_then(|v| v.split_once(':').map(|(a, b)| (a.to_string(), b.parse::<u32>().unwrap_or(1)))) {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        p.perm_levels.retain(|(k, _)| k != &id);
        p.perm_levels.push((id.clone(), rank));
        p.equipped_ability = id;
    }
    // As the app does at start.
    crate::scenes::game::build_scene::enable_game_lighting(&mut canvas);
    // `HEADLESS_NO_PRELOAD=1`: do not enter the game first, so the first
    // entry happens the way a player's does (from the menu).
    if std::env::var("HEADLESS_NO_PRELOAD").is_err() {
        canvas.load_scene("game");
    }
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

/// `HEADLESS_SHOT_DIR=<dir>` (every `HEADLESS_SHOT_EVERY` frames, default
/// 600): render the frame through the real draw tree — the same items the
/// device draws, post-processing included — and save it half size as
/// `<dir>/frame_NNNNNN.png`. So visual work can be looked at without a
/// device in hand.
/// `HEADLESS_SHOP=<dir>`: open the shop the way the menu does and capture
/// the look screens — the cat carousel, and two rope styles with the colour
/// row — so shop changes can be reviewed without a device. Saves go to the
/// headless temp dir (see `build_canvas`), never a real profile.
/// `HEADLESS_MENU=<dir>`: the main menu as the game draws it, so its look
/// can be reviewed without a device: frames over its first three seconds
/// (anything animated shows as it moves), saved as `<dir>/frame_NNNNNN.png`.
pub fn capture_menu(dir: &str) {
    let (mut ctx, _recv) = prism::Context::new();
    let mut canvas = build_canvas(&mut ctx, 0.0);
    let sized = SizedTree::default();
    // `HEADLESS_SCENE=<name>`: shoot that scene (settings, profile, ...) instead.
    let scene = std::env::var("HEADLESS_SCENE").unwrap_or_else(|_| "menu".into());
    canvas.load_scene(&scene);
    // `HEADLESS_VAR_F32=name=value`: set a float var after loading (e.g. a
    // page's scroll target), to shoot a state the page reaches by input.
    if let Some((k, v)) = std::env::var("HEADLESS_VAR_F32").ok().as_deref().and_then(|s| s.split_once('=')) {
        if let Ok(v) = v.parse::<f32>() { canvas.set_var(k, v); }
    }
    // `HEADLESS_MENU_TO_SHOP=1`: open the shop at tick 60 and film the trip
    // up into it, then the categories, then the CHARACTERS carousel.
    let to_shop = std::env::var("HEADLESS_MENU_TO_SHOP").is_ok();
    // `HEADLESS_EVENT=<custom event>@<tick>`: fire it then (e.g. goto_game@60);
    // `HEADLESS_SHOT_STEP=<n>` films every n ticks instead of every 30.
    let event = std::env::var("HEADLESS_EVENT").ok().and_then(|v| {
        let (n, t) = v.split_once('@')?;
        Some((n.to_string(), t.parse::<u64>().ok()?))
    });
    let step: u64 = std::env::var("HEADLESS_SHOT_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(30).max(1);
    // `HEADLESS_TICKS=<n>`: run that many ticks (default 180).
    let ticks: u64 = std::env::var("HEADLESS_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(180);
    for i in 1..=ticks {
        let frame0 = std::time::Instant::now();
        if let Some((name, at)) = &event {
            if i == *at { canvas.run(Action::Custom { name: name.clone() }); }
        }
        // `HEADLESS_GO=<scene>@<tick>`: the slow shutter to a scene then.
        if let Some((sc, at)) = std::env::var("HEADLESS_GO").ok().as_deref().and_then(|v| v.split_once('@')).and_then(|(n, t)| Some((n.to_string(), t.parse::<u64>().ok()?))) {
            if i == at { crate::transition::go_slow(&mut canvas, &sc); }
        }
        // `HEADLESS_EVENT2=<event>@<tick>`: a second one.
        if let Some((name, at)) = std::env::var("HEADLESS_EVENT2").ok().as_deref().and_then(|v| v.split_once('@')).and_then(|(n, t)| Some((n.to_string(), t.parse::<u64>().ok()?))) {
            if i == at { canvas.run(Action::Custom { name }); }
        }
        // `HEADLESS_FREEZE_ORBIT=<tick>`: hold the start screen's orbit at
        // that point, so the cat sits still (filming the start screen).
        if let Some(at) = std::env::var("HEADLESS_FREEZE_ORBIT").ok().and_then(|v| v.parse::<i32>().ok()) {
            if canvas.get_bool("start_prompt_active") { canvas.set_var("start_orbit_ticks", at); }
        }
        let tick0 = std::time::Instant::now();
        // `HEADLESS_SPACE_AT=<tick>`: press (and hold) Space then.
        if std::env::var("HEADLESS_SPACE_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(i) {
            send_key(&mut ctx, &mut canvas, &sized, KeyboardState::Pressed);
        }
        if std::env::var("TIMING_DBG").is_ok() { let e = tick0.elapsed(); if e.as_millis() > 40 { eprintln!("timing: tick {i} took {e:?}"); } }
        if std::env::var("HEADLESS_TRACE").is_ok() && i % 10 == 0 {
            let p = canvas.get_game_object("player").map(|o| (o.position, o.visible));
            eprintln!("t={i} scene_game={} paused={} player={p:?} cam={:?} intro={} prompt={} hold={} space={} gp={}",
                canvas.is_scene("game"), canvas.is_paused(),
                canvas.camera().map(|c| (c.position, c.zoom)), canvas.get_i32("game_intro_t"),
                canvas.get_bool("start_prompt_active"), canvas.get_i32("start_hold_ticks"), canvas.key("space"), canvas.get_bool("game_paused"));
        }
        if to_shop && i == 60 {
            canvas.set_var("menu_cam_target_y", 0.0f32);
            canvas.set_var("menu_in_shop", true);
            crate::shop::init_shop(&mut canvas);
        }
        OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);
        // Lay out every tick, as a window does: sizes come from the last
        // layout, so laying out only on the frames shot left a fast-moving
        // camera's frames drawn at a stale scale.
        {
            use prism::drawable::Drawable;
            let screen = (VW, VH);
            let req = canvas.request_size();
            let _ = canvas.build(screen, req);
        }
        if if to_shop { i >= 56 && i % 4 == 0 } else { i % step == 0 } {
            shoot_frame(&mut canvas, dir, i);
        }
        // `HEADLESS_FRAME_MS=<ms>`: report any whole frame (tick + layout +
        // shot) slower than that — first-time stalls show up here.
        if let Some(lim) = std::env::var("HEADLESS_FRAME_MS").ok().and_then(|v| v.parse::<u128>().ok()) {
            let e = frame0.elapsed();
            if e.as_millis() > lim { eprintln!("slow frame {i}: {e:?} scene_game={}", canvas.is_scene("game")); }
        }
    }
    if to_shop {
        // `HEADLESS_SHOP_CAT=<n>`: which category's carousel to end on.
        let cat = std::env::var("HEADLESS_SHOP_CAT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        crate::shop::show_carousel(&mut canvas, cat);
        for i in 181..=240u64 {
            OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);
            if i == 240 { shoot_frame(&mut canvas, dir, i); }
        }
    }
}

pub fn capture_shop(dir: &str) {
    let (mut ctx, _recv) = prism::Context::new();
    let mut canvas = build_canvas(&mut ctx, 0.0);
    let sized = SizedTree::default();
    canvas.load_scene("menu");
    {
        let g = crate::profile::profile();
        g.lock().unwrap().meta_currency = 500;
    }
    canvas.set_var("menu_cam_target_y", 0.0f32);
    canvas.set_var("menu_in_shop", true);
    crate::shop::init_shop(&mut canvas);
    // `HEADLESS_SHOP_SEQ=<rope style>`: instead of the four stills, film that
    // style's ROPES preview — 24 frames, one every 2 ticks — to judge its
    // motion (loop pops, stutters, seams) frame by frame.
    if let Some(style) = std::env::var("HEADLESS_SHOP_SEQ").ok().and_then(|v| v.parse::<i32>().ok()) {
        crate::shop::show_carousel(&mut canvas, 1);
        canvas.set_var("player_rope_selected", 0i32);
        canvas.set_var("shop_selected", style);
        crate::shop::refresh_carousel(&mut canvas, 1, style as usize);
        for i in 0..(30 + 24 * 2) {
            OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);
            if i >= 30 && (i - 30) % 2 == 0 {
                shoot_frame(&mut canvas, dir, (i - 30) as u64 / 2 + 1);
            }
        }
        return;
    }
    let mut shots: Vec<(i32, i32, i32, u64)> = vec![(0, 15, 0, 1), (0, 8, 0, 2), (1, 1, 5, 3), (1, 7, 8, 4)];
    // `HEADLESS_SHOP_CAT=<category>`: that category's carousel instead, at
    // its first and fourth items.
    if let Some(cat) = std::env::var("HEADLESS_SHOP_CAT").ok().and_then(|v| v.parse::<i32>().ok()) {
        shots = vec![(cat, 0, 0, 1), (cat, 3, 0, 2)];
    }
    for (cat, sel, color, n) in shots {
        crate::shop::show_carousel(&mut canvas, cat);
        canvas.set_var("player_rope_selected", color);
        canvas.set_var("shop_selected", sel);
        crate::shop::refresh_carousel(&mut canvas, cat, sel as usize);
        for _ in 0..90 {
            OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);
        }
        shoot_frame(&mut canvas, dir, n);
    }
}

/// `HEADLESS_LABEL=<text>@<path>`: write `text` in the menu's glowing title
/// style to a PNG (labels for the shop's upgrade demo clips).
pub fn write_label(spec: &str) {
    let Some((text, path)) = spec.split_once('@') else { return };
    let px: f32 = std::env::var("HEADLESS_LABEL_PX").ok().and_then(|v| v.parse().ok()).unwrap_or(64.0);
    if let Some(img) = crate::menu_look::glow_text(text, px) {
        let _ = img.save(path);
    }
}

/// `HEADLESS_SHOW_RING=reach|magnet`: a ring around the player at that
/// radius, so a clip can SHOW a reach or pickup range.
fn place_demo_ring(canvas: &mut Canvas) {
    let Ok(kind) = std::env::var("HEADLESS_SHOW_RING") else { return };
    let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() else { return };
    let (r, (px, py), col) = {
        let mut s = st.lock().unwrap();
        // `HEADLESS_RING_GROW=<frame>`: the reach starts at the BASE reach and
        // grows to this run's over the half second from then — the line
        // visibly getting longer, and the node lighting up the moment it is
        // in reach, as it does in play.
        if kind != "magnet" {
            if let Some(g) = std::env::var("HEADLESS_RING_GROW").ok().and_then(|v| v.parse::<u64>().ok()) {
                static FULL: std::sync::Mutex<Option<f32>> = std::sync::Mutex::new(None);
                let full = *FULL.lock().unwrap().get_or_insert(s.perm_reach_mult);
                let f = crate::scenes::game::boss::fx_kit::FRAME.load(std::sync::atomic::Ordering::Relaxed);
                let k = ((f.saturating_sub(g)) as f32 / 30.0).clamp(0.0, 1.0);
                let k = k * k * (3.0 - 2.0 * k);
                s.perm_reach_mult = 1.0 + (full - 1.0) * k;
            }
        }
        let r = if kind == "magnet" { crate::gameplay::player_magnet_radius(&s) } else { crate::gameplay::player_grab_reach(&s) };
        let col = if kind == "magnet" { [255, 220, 90] } else { [130, 210, 255] };
        (r, (s.px, s.py), col)
    };
    if canvas.get_game_object("demo_ring").is_none() {
        let n = 256u32;
        let img = image::RgbaImage::from_fn(n, n, |x, y| {
            let d = ((x as f32 + 0.5 - 128.0).powi(2) + (y as f32 + 0.5 - 128.0).powi(2)).sqrt();
            let a = (1.0 - ((d - 124.0).abs() / 3.0)).clamp(0.0, 1.0) * 230.0 + if d < 124.0 { 28.0 } else { 0.0 };
            image::Rgba([col[0], col[1], col[2], a as u8])
        });
        let o = GameObject::build("demo_ring").size(8.0, 8.0)
            .image(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: std::sync::Arc::new(img), color: None })
            .layer(9_000).finish();
        canvas.add_game_object("demo_ring".into(), o);
    }
    if let Some(o) = canvas.get_game_object_mut("demo_ring") {
        o.size = (r * 2.0, r * 2.0);
        o.position = (px - r, py - r);
        o.visible = true;
        o.update_image_shape();
    }
}

fn shoot_frame(canvas: &mut Canvas, dir: &str, frame: u64) {
    place_demo_ring(canvas);
    use prism::drawable::Drawable;
    thread_local! {
        static RENDERER: std::cell::RefCell<Option<prism::canvas::HeadlessRenderer>> =
            const { std::cell::RefCell::new(None) };
    }
    // `HEADLESS_SHOT_W=<px>`: render straight at that width (16:9) instead of
    // 4K-then-halved — for filming clips, where 4K per frame is far too slow.
    let small = std::env::var("HEADLESS_SHOT_W").ok().and_then(|v| v.parse::<f32>().ok());
    let screen = match small {
        Some(w) => (w, (w * 9.0 / 16.0).round()),
        None => (crate::constants::VW, crate::constants::VH),
    };
    let req = canvas.request_size();
    let sized = canvas.build(screen, req);
    let items = canvas.draw(&sized, (0.0, 0.0), (0.0, 0.0, screen.0, screen.1));
    let rgba = RENDERER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = block_on(prism::canvas::HeadlessRenderer::new(screen.0 as u32, screen.1 as u32)).ok();
        }
        slot.as_mut().map(|r| r.render(items))
    });
    let Some((w, h, px)) = rgba else {
        eprintln!("[SHOT] no GPU available; frame {frame} not captured");
        return;
    };
    let Some(img) = image::RgbaImage::from_raw(w, h, px) else { return };
    let half = if small.is_some() { img } else {
        image::imageops::resize(&img, w / 2, h / 2, image::imageops::FilterType::Triangle)
    };
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{dir}/frame_{frame:06}.png");
    match half.save(&path) {
        Ok(()) => eprintln!("[SHOT] {path}"),
        Err(e) => eprintln!("[SHOT] {path}: {e}"),
    }
}

/// Drive a future to completion on this thread. `pollster` is a
/// dev-dependency only, and this is all of it that the capture needs.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
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

/// `HEADLESS_STAGE=<frame>` with `HEADLESS_STAGE_KIND=reach|swing`: a clean
/// stage for filming one upgrade's demo. From that frame everything in the
/// world near the player — nodes, coins, bounce pads, cannons, spinners — is
/// parked out of sight every tick, except the stage's own nodes, and the bot
/// stops steering (space is held, so a rope once caught stays caught).
///
/// * `reach` (LONG LINE): the player hangs still with two nodes drifting in
///   from either side; the game's own grab runs every tick, so the rope fires
///   the moment one crosses the reach. Then the player swings.
/// * `swing` (FLYWHEEL): the player catches a node level with it and is kicked
///   far past any cap, so the cap is what sets the speed; a speed meter (this
///   clip's alone — never in the game) shows the speed and the top speed.
///
/// Returns whether the stage is running (the bot's input is then overridden).
#[derive(Default)]
struct Stage { center: (f32, f32), hooks: Vec<String>, spots: Vec<(f32, f32)>, grabbed: bool, kicked: u32, peak: f32, released: bool, hooked_ticks: u32 }
static STAGE: std::sync::Mutex<Option<Stage>> = std::sync::Mutex::new(None);

/// After each tick, while a stage runs: the camera held on it
/// (`HEADLESS_STAGE_ZOOM`, default 0.8), so both sides are in shot and the
/// view does not chase the swing.
fn stage_camera(canvas: &mut Canvas) {
    let mut guard = STAGE.lock().unwrap();
    let Some(g) = guard.as_mut() else { return };
    let (cx, cy) = g.center;
    // A swing is framed on its node, so the whole loop is in shot.
    let center = if std::env::var("HEADLESS_STAGE_KIND").map_or(false, |k| k == "swing") {
        g.spots.first().copied().unwrap_or(g.center)
    } else {
        g.center
    };
    // The stage's nodes: where the stage says, every tick (the game recycles
    // and bobs its nodes, and would carry these off).
    for (h, (x, y)) in g.hooks.iter().zip(g.spots.iter()) {
        if let Some(o) = canvas.get_game_object_mut(h) {
            o.position = (x - o.size.0 * 0.5, y - o.size.1 * 0.5);
            o.momentum = (0.0, 0.0);
            o.visible = true;
        }
    }

    // Park the rest of the world.
    let nearby: Vec<String> = match canvas.get_game_object("player") {
        Some(p) => canvas.objects_in_radius(p, 9000.0).into_iter().filter(|o| {
            let t = |n: &str| o.tags.iter().any(|x| x == n);
            (t("hook") || t("coin") || t("pad") || t("pad_thruster") || t("gravity_cannon") || t("upgrade_node") || t("decor")
                || ["spinner", "gwell", "turret", "comet"].iter().any(|p| o.id.starts_with(p)))
                && !g.hooks.contains(&o.id)
        }).map(|o| o.id.clone()).collect(),
        None => Vec::new(),
    };
    // And every live node but the stage's own, wherever it is (a node parked
    // last tick is out of that radius, and the world would bring it back).
    let mut nearby = nearby;
    if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
        for h in st.lock().unwrap().live_hooks.iter() {
            if !g.hooks.contains(h) && !nearby.contains(h) { nearby.push(h.clone()); }
        }
    }
    for id in nearby {
        if let Some(o) = canvas.get_game_object_mut(&id) {
            o.visible = false;
            o.position = (cx, cy + 2.0e6);
        }
    }

    // The swing's meter, read AFTER the tick: the game's speed cap has been
    // applied by then (before it, the stage's push is still in there).
    if std::env::var("HEADLESS_STAGE_KIND").map_or(false, |k| k == "swing") {
        if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
            let (vx, vy) = { let s = st.lock().unwrap(); (s.vx, s.vy) };
            let v = (vx * vx + vy * vy).sqrt();
            if g.grabbed && g.hooked_ticks > 2 { g.peak = g.peak.max(v); }
            let (show, peak) = (if g.grabbed { v } else { 0.0 }, g.peak);
            speed_meter(canvas, show, peak);
        }
    }
    // The static stage: the cat curled (its swinging poses would differ
    // between the two halves of the clip).
    if std::env::var("HEADLESS_STAGE_KIND").map_or(false, |k| k == "static") {
        if let Some(o) = canvas.get_game_object_mut("player") {
            if let Some(a) = o.animated_sprite.as_mut() { a.set_frame(0); }
        }
    }
    // Still hanging: held where the stage began (the tick's gravity undone).
    if !g.grabbed {
        if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
            let mut s = st.lock().unwrap();
            if std::env::var("HEADLESS_STAGE_KIND").map_or(false, |k| k == "static") {
                s.hooked = false;
                s.active_hook = String::new();
            }
            if !s.hooked { s.px = cx; s.py = cy; s.vx = 0.0; s.vy = 0.0; }
        }
        if let Some(o) = canvas.get_game_object_mut("player") {
            o.position = (cx - PLAYER_R, cy - PLAYER_R);
            o.momentum = (0.0, 0.0);
        }
    }
    drop(guard);
    let z: f32 = std::env::var("HEADLESS_STAGE_ZOOM").ok().and_then(|v| v.parse().ok()).unwrap_or(0.8);
    if let Some(cam) = canvas.camera_mut() {
        cam.follow(None);
        cam.zoom_anchor = None;
        cam.lerp_speed = 1.0;
        cam.zoom = z;
        cam.zoom_target = z;
        cam.position = (center.0 - VW * 0.5 / z, center.1 - VH * 0.5 / z);
    }
}

fn stage_tick(canvas: &mut Canvas, ctx: &mut prism::Context, sized: &SizedTree, frames: u64) -> bool {
    let Some(at) = std::env::var("HEADLESS_STAGE").ok().and_then(|v| v.parse::<u64>().ok()) else { return false };
    if frames < at { return false; }
    let kind = std::env::var("HEADLESS_STAGE_KIND").unwrap_or_else(|_| "reach".into());
    let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() else { return false };
    let mut guard = STAGE.lock().unwrap();
    if frames == at || guard.is_none() {
        let s = st.lock().unwrap();
        if std::env::var("STAGE_DBG").is_ok() {
            eprintln!("STAGE live {} first {:?}", s.live_hooks.len(),
                s.live_hooks.iter().take(4).map(|h| canvas.get_game_object(h).map(|o| o.tags.clone())).collect::<Vec<_>>());
        }
        let want = if kind == "reach" { 2 } else { 1 };
        let hooks: Vec<String> = s.live_hooks.iter().filter(|h| {
            canvas.get_game_object(h).map_or(false, |o| {
                let t = |n: &str| o.tags.iter().any(|x| x == n);
                t("hook") && !t("checkpoint_node") && !t(SPECIAL_HOOK_TAG) && !t(EXTENDED_HOOK_TAG)
            })
        }).take(want).cloned().collect();
        // A fixed height in the play band, not wherever the bot happened to
        // be (mid-fall, or below the screen respawning).
        *guard = Some(Stage { center: (s.px, 640.0), hooks, ..Default::default() });
        drop(s);
        // Drop the rope the bot was on: the stage starts in the air.
        {
            let mut s = st.lock().unwrap();
            s.hooked = false;
            s.active_hook = String::new();
            s.respawn_active = false;
        }
        canvas.run(Action::Hide { target: Target::name("rope") });
        let g = guard.as_mut().unwrap();
        let (cx, cy) = g.center;
        let spots: Vec<(f32, f32)> = if kind == "swing" {
            // Up and ahead, inside any reach.
            vec![(cx + 460.0, cy - 460.0)]
        } else if kind == "static" {
            // LONG LINE: one node right on the BASE reach, so the base ring's
            // edge just touches it and an upgraded ring takes it in.
            // Just past the BASE reach: the base ring's edge just short of it
            // (no highlight); an upgraded ring takes it in, and it lights.
            vec![(cx + ROPE_LEN_MAX + 40.0, cy)]
        } else {
            vec![(cx + 1500.0, cy - 250.0), (cx - 1500.0, cy - 250.0)]
        };
        g.spots = spots.into_iter().take(g.hooks.len()).collect();
    }
    let g = guard.as_mut().unwrap();
    let (cx, cy) = g.center;
    if std::env::var("STAGE_DBG").is_ok() && frames % 10 == 0 {
        let prox = match canvas.get_var("hook_prox_id") { Some(Value::Str(v)) => v, _ => String::new() };
        let sp = g.hooks.first().and_then(|h| canvas.get_game_object(h)).and_then(|o| o.animated_sprite.as_ref().map(|a| a.current_frame_index()));
        let mult = st.lock().unwrap().perm_reach_mult;
        eprintln!("HL f{frames} staged {:?} prox {prox:?} frame {sp:?} mult {mult:.2}", g.hooks.first());
    }
    if std::env::var("STAGE_DBG").is_ok() && frames % 20 == 0 {
        let hs: Vec<_> = g.hooks.iter().map(|h| canvas.get_game_object(h).map(|o| (o.position, o.visible))).collect();
        eprintln!("STAGE f{frames} center {:?} hooks {:?} {hs:?}", g.center, g.hooks);
    }

    // The stage's nodes stay LIVE: the world recycles nodes the bot has left
    // behind, and a node not in the live list is never highlighted or caught.
    {
        let mut s = st.lock().unwrap();
        for h in &g.hooks { if !s.live_hooks.contains(h) { s.live_hooks.push(h.clone()); } }
    }
    // Only the stage's own node counts: a rope still on one of the world's
    // (a grab already in flight when the stage began) is cut.
    {
        let mut s = st.lock().unwrap();
        if s.hooked && !g.hooks.contains(&s.active_hook) && !g.grabbed {
            s.hooked = false;
            s.active_hook = String::new();
        }
    }
    let hooked = st.lock().unwrap().hooked;
    if hooked { g.grabbed = true; }
    if !g.grabbed {
        // Hang still where the stage began.
        {
            let mut s = st.lock().unwrap();
            s.px = cx; s.py = cy; s.vx = 0.0; s.vy = 0.0;
        }
        if let Some(o) = canvas.get_game_object_mut("player") {
            o.position = (cx - PLAYER_R, cy - PLAYER_R);
            o.momentum = (0.0, 0.0);
        }
        if kind == "reach" {
            let drift: f32 = std::env::var("HEADLESS_STAGE_DRIFT").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
            for p in g.spots.iter_mut() { p.0 += if p.0 > cx { -drift } else { drift }; }
        }
        if frames > at + 10 && kind != "static" { canvas.run(Action::Custom { name: "do_grab".into() }); }
    } else if kind == "swing" && !g.released {
        // On the rope: driven along the swing, far past any cap, for the
        // first stretch, so the cap is what sets the speed; then released
        // as the swing carries it up and forward.
        g.hooked_ticks += 1;
        let (px, py, hx, hy, vx, vy) = { let s = st.lock().unwrap(); (s.px, s.py, s.hook_x, s.hook_y, s.vx, s.vy) };
        let (rx, ry) = (px - hx, py - hy);
        let len = (rx * rx + ry * ry).sqrt().max(1.0);
        if std::env::var("STAGE_DBG").is_ok() && g.hooked_ticks % 5 == 0 {
            let s2 = st.lock().unwrap();
            eprintln!("SWING t{} rel ({rx:.0},{ry:.0}) v ({vx:.1},{vy:.1}) hook {} dlg {} hold {} cap {} gp {} paused {}", g.hooked_ticks,
                s2.active_hook, s2.upgrade_dialogue_active, s2.upgrade_hold_until_tether, s2.cannon_captured,
                canvas.get_bool("game_paused"), canvas.is_paused());
        }
        if g.hooked_ticks < 14 {
            // Tangent pointing down-and-forward from the start (clockwise).
            let t = (-ry / len, rx / len);
            let t = if t.0 > 0.0 || (t.0 == 0.0 && t.1 > 0.0) { t } else { (-t.0, -t.1) };
            let t = if ry < 0.0 { (-t.0, -t.1) } else { t };
            let kick = (t.0 * 160.0, t.1 * 160.0);
            { let mut s = st.lock().unwrap(); s.vx = kick.0; s.vy = kick.1; }
            if let Some(o) = canvas.get_game_object_mut("player") { o.momentum = kick; }
        } else if (vx > 0.0 && vy < 0.0 && rx > 0.0 && ry < 260.0) || g.hooked_ticks > 200 {
            // Up the far side, moving up and forward: let go.
            g.released = true;
        }
    }
    let _ = (ctx, sized);

    true
}

/// The FLYWHEEL clip's speed meter: a bar, the speed, and the top speed.
fn speed_meter(canvas: &mut Canvas, v: f32, peak: f32) {
    const FULL: f32 = 80.0;
    let (bw, bh) = (1800.0f32, 70.0f32);
    let (bx, by) = ((VW - bw) * 0.5, VH - 520.0);
    let solid = |r: u8, g: u8, b: u8, a: u8| std::sync::Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba([r, g, b, a])));
    let mk = |canvas: &mut Canvas, id: &str, img: std::sync::Arc<image::RgbaImage>| {
        if canvas.get_game_object(id).is_none() {
            let mut o = GameObject::build(id).size(8.0, 8.0)
                .image(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: img, color: None })
                .layer(9_100).ignore_zoom().finish();
            o.unlit = true;
            o.gravity = 0.0;
            canvas.add_game_object(id.into(), o);
        }
    };
    mk(canvas, "demo_meter_bg", solid(20, 16, 40, 210));
    mk(canvas, "demo_meter_fill", solid(255, 190, 70, 255));
    mk(canvas, "demo_meter_peak", solid(255, 255, 255, 255));
    let place = |canvas: &mut Canvas, id: &str, pos: (f32, f32), size: (f32, f32)| {
        if let Some(o) = canvas.get_game_object_mut(id) {
            o.position = pos; o.size = size; o.visible = size.0 > 0.5;
        }
    };
    let pad = 10.0;
    place(canvas, "demo_meter_bg", (bx - pad, by - pad), (bw + pad * 2.0, bh + pad * 2.0));
    let fw = (v / FULL).clamp(0.0, 1.0) * bw;
    place(canvas, "demo_meter_fill", (bx, by), (fw, bh));
    let px = bx + (peak / FULL).clamp(0.0, 1.0) * bw;
    place(canvas, "demo_meter_peak", (px - 6.0, by - 24.0), (if peak > 0.0 { 12.0 } else { 0.0 }, bh + 48.0));
    for o in ["demo_meter_bg", "demo_meter_fill", "demo_meter_peak"] {
        if let Some(obj) = canvas.get_game_object_mut(o) {
            let size = obj.size;
            let img = match o { "demo_meter_bg" => solid(20, 16, 40, 210), "demo_meter_fill" => solid(255, 190, 70, 255), _ => solid(255, 255, 255, 255) };
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None });
        }
    }
    let label = format!("SPEED {:>3}     TOP {:>3}", v.round() as i32, peak.round() as i32);
    if let Some(img) = crate::menu_look::glow_text(&label, 120.0) {
        let size = (img.width() as f32, img.height() as f32);
        mk(canvas, "demo_meter_txt", img.clone());
        if let Some(o) = canvas.get_game_object_mut("demo_meter_txt") {
            o.size = size;
            o.position = ((VW - size.0) * 0.5, by - size.1 - 10.0);
            o.visible = true;
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None });
        }
    }
}

fn zone_for_distance(d: f32) -> usize {
    crate::difficulty::zone_index_for_distance(d)
}

// ── Episode runner ────────────────────────────────────────────────────────────

fn run_episode(max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool, weakpoint_check: bool, stasis_down: bool, flare_test: bool, shelter_check: bool, start_minute: f32) -> EpisodeReport {
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
        // Validate the existing Sun Devourer fight specifically, not whichever
        // boss currently occupies roster slot 0 (the new bosses land there once
        // implemented).
        canvas.set_var("debug_boss_kind_sundev", true);
    }
    if std::env::var("HEADLESS_IMMORTAL").is_ok() {
        // Screenshots of a long run need the run to last: falls respawn
        // instead of ending it.
        canvas.set_var("debug_immortal", true);
    }
    if stasis_down {
        // Start whichever boss occupies the current roster slot (e.g. the
        // Colossus at index 0) deterministically, without forcing the Sun
        // Devourer. Lets the harness drive the multi-part bosses.
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
    let mut buff_desync_frames: u64 = 0;
    let mut airborne_frames: u64 = 0;
    let mut starve_streak: u64 = 0;
    let mut worst_starve_streak: u64 = 0;
    let mut flares_without_shelter: i32 = 0;
    let mut shelter_dx_sum = 0.0f32;
    let mut shelter_dx_worst = 0.0f32;
    let mut prev_flare_count: i32 = 0;
    let mut census = HazardCensus::default();
    let mut worst_overshoot = 0.0f32;

    let shot_dir = std::env::var("HEADLESS_SHOT_DIR").ok();
    let shot_every: u64 = std::env::var("HEADLESS_SHOT_EVERY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
        .max(1);
    // `HEADLESS_SHOT_FROM=<frame>`: no shots before it (film a window).
    let shot_from: u64 = std::env::var("HEADLESS_SHOT_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let cine_at: Option<u64> = std::env::var("HEADLESS_PHASE_CINE").ok().and_then(|v| v.parse().ok());
    let ab_at: Option<u64> = std::env::var("HEADLESS_AB_AT").ok().and_then(|v| v.parse().ok());
    // `HEADLESS_KILL_AT=<frame>`: spend every heart then; `HEADLESS_DEATH_HOLD=
    // <frames>`: keep running that long after dying (filming the death screen).
    let kill_at: Option<u64> = std::env::var("HEADLESS_KILL_AT").ok().and_then(|v| v.parse().ok());
    let death_hold: u64 = std::env::var("HEADLESS_DEATH_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut died_at: Option<u64> = None;
    while frames < max_frames {
        crate::scenes::game::boss::fx_kit::FRAME.store(frames, std::sync::atomic::Ordering::Relaxed);
        if let Some(dir) = &shot_dir {
            if frames > 0 && frames % shot_every == 0 && frames >= shot_from {
                shoot_frame(&mut canvas, dir, frames);
            }
        }
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
            // The buff's two fields must never disagree: `player_buff > 0` is
            // what every consumer tests, `buff_timer` is what expires it.
            {
                let pb = get_i32_or(&canvas, "player_buff", 0);
                let bt = get_i32_or(&canvas, "buff_timer", 0);
                if pb > 0 && bt <= 0 {
                    buff_desync_frames += 1;
                }
            }

            // Count what the draw tree would actually emit, mirroring what
            // ramp does each frame, so the offscreen cull can be measured
            // rather than asserted.
            if std::env::var("QUARTZ_COUNT_ITEMS").is_ok() && frames % 300 == 0 {
                use prism::drawable::Drawable;
                let screen = (crate::constants::VW, crate::constants::VH);
                let req = canvas.request_size();
                let sized = canvas.build(screen, req);
                let items = canvas.draw(&sized, (0.0, 0.0), (0.0, 0.0, screen.0, screen.1));
                let objects = canvas.object_count();
                eprintln!("[ITEMS] frame={frames} objects={objects} items={}", items.len());
            }

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
                // Every pooled hook, hidden or not: a node spawns hidden,
                // off-screen, and animates in as the player nears it, so a
                // visibility-filtered search reported live shelters 5 000 px
                // ahead as missing (and nearly every late-run flare as a
                // violation). A hidden node's x is already its final x.
                let px = canvas
                    .get_game_object("player")
                    .map(|p| p.position.0 + p.size.0 * 0.5)
                    .unwrap_or(0.0);
                let nearest = (0..crate::constants::HOOK_POOL_SIZE)
                    .filter_map(|i| canvas.get_game_object(&format!("hook_{i}")))
                    .filter(|o| o.tags.iter().any(|t| t == crate::constants::SHIELD_HOOK_TAG))
                    .map(|o| o.position.0 + o.size.0 * 0.5 - px)
                    .filter(|dx| {
                        *dx <= crate::constants::FLARE_SHELTER_SEARCH_AHEAD
                            && *dx >= -crate::constants::FLARE_SHELTER_SEARCH_BEHIND
                    })
                    .map(f32::abs)
                    .fold(None, |best: Option<f32>, d| Some(best.map_or(d, |b| b.min(d))));
                let shelter_near = nearest.is_some();
                if let Some(d) = nearest {
                    shelter_dx_sum += d;
                    shelter_dx_worst = shelter_dx_worst.max(d);
                }
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

        // `HEADLESS_GOD_AT=<frame>`: press 0 (god mode: the player stops
        // dead, the rope goes) on that frame, and the bot lets go for good —
        // for filming the HUD with nothing moving.
        let god_at: Option<u64> = std::env::var("HEADLESS_GOD_AT").ok().and_then(|v| v.parse().ok());
        if god_at == Some(frames) {
            for state in [KeyboardState::Pressed, KeyboardState::Released] {
                let evt = KeyboardEvent { key: Key::Character("0".into()), state, modifiers: Modifiers::none() };
                OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(evt) as Box<dyn Event>);
            }
        }
        let (hold, target) = if god_at.map_or(false, |g| frames >= g) { (false, None) } else { (hold, target) };
        // `HEADLESS_FLARE_AT=<frame>`: a solar flare goes off then (filming it).
        if std::env::var("HEADLESS_FLARE_AT").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frames) {
            canvas.set_var("debug_flare_interval", 100_000i32);
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                let mut s = st.lock().unwrap();
                s.flare_warn = 0;
                s.flare_active = true;
                // `HEADLESS_FLARE_TICKS=<n>`: a shorter (or longer) flare.
                s.flare_active_ticks = std::env::var("HEADLESS_FLARE_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(FLARE_ACTIVE_TICKS);
                // As a real eruption: the first burn a second in, the
                // SUNPROOFING wards full.
                s.flare_damage_timer = FLARE_DAMAGE_GRACE;
                s.flare_wards_left = s.perm_flare_wards;
                // `HEADLESS_HEARTS=<n>`: hearts left as it goes off.
                if let Some(n) = std::env::var("HEADLESS_HEARTS").ok().and_then(|v| v.parse::<i32>().ok()) {
                    s.hearts = n.min(s.max_hearts);
                }
            }
        }
        let staged = stage_tick(&mut canvas, &mut ctx, &sized, frames);
        let released = STAGE.lock().unwrap().as_ref().map_or(false, |g| g.released);
        let static_stage = std::env::var("HEADLESS_STAGE_KIND").map_or(false, |k| k == "static");
        let (hold, target) = if staged { (!static_stage && !released, None) } else { (hold, target) };
        // `HEADLESS_COIN_ROW=<frame>`: lay coins out in a row either side of
        // the player then, well outside any pickup range, and drift them in
        // (`HEADLESS_COIN_DRIFT` px a tick, default 5) — each is snatched the
        // moment it crosses the range, which SHOWS how far MAGNETISM reaches.
        // Pair it with HEADLESS_GOD_AT so the player holds still.
        if let Some(row_at) = std::env::var("HEADLESS_COIN_ROW").ok().and_then(|v| v.parse::<u64>().ok()) {
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                static ROW: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
                let (px, py) = { let s = st.lock().unwrap(); (s.px, s.py) };
                if frames == row_at {
                    let live = {
                        let mut s = st.lock().unwrap();
                        s.coin_magnet_locked.clear();
                        s.coin_live.clone()
                    };
                    let mut spots = Vec::new();
                    for d in [420.0f32, 540.0, 660.0] { spots.push((px + d, py)); spots.push((px - d, py)); }
                    let mut row = ROW.lock().unwrap();
                    row.clear();
                    for (id, (x, y)) in live.iter().zip(spots) {
                        if let Some(o) = canvas.get_game_object_mut(id) {
                            o.position = (x - COIN_R, y - COIN_R);
                            o.visible = true;
                            row.push(id.clone());
                        }
                    }
                } else if frames > row_at {
                    let drift: f32 = std::env::var("HEADLESS_COIN_DRIFT").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
                    let locked = st.lock().unwrap().coin_magnet_locked.clone();
                    for id in ROW.lock().unwrap().iter() {
                        if locked.contains(id) { continue; }
                        if let Some(o) = canvas.get_game_object_mut(id) {
                            let cx = o.position.0 + COIN_R;
                            o.position.0 += if cx > px { -drift } else { drift };
                        }
                    }
                }
            }
        }
        if let Some((tx, ty)) = target {
            canvas.set_var("mouse_grab_x", Value::F32(tx));
            canvas.set_var("mouse_grab_y", Value::F32(ty));
        }
        if hold != space_held {
            let state = if hold { KeyboardState::Pressed } else { KeyboardState::Released };
            send_key(&mut ctx, &mut canvas, &sized, state);
            space_held = hold;
        }

        if kill_at == Some(frames) {
            canvas.set_var("debug_immortal", false);
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                let mut s = st.lock().unwrap();
                s.hearts = 0;
                s.free_respawns_left = 0;
            }
        }

        // `HOOK_DUMP=<frame>`: print every live node's centre, sorted by x,
        // with the gap to the previous one (measuring node spacing).
        if std::env::var("HOOK_DUMP").ok().and_then(|v| v.parse::<u64>().ok()) == Some(frames) {
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                let ids = st.lock().unwrap().live_hooks.clone();
                let mut pts: Vec<(f32, f32, String)> = ids.iter().filter_map(|id| canvas.get_game_object(id)
                    .filter(|o| o.visible).map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5, o.tags.join("|")))).collect();
                pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let mut prev: Option<(f32, f32)> = None;
                for (x, y, tags) in &pts {
                    let d = prev.map(|(px, py)| ((x - px).powi(2) + (y - py).powi(2)).sqrt()).unwrap_or(0.0);
                    eprintln!("HOOK x={x:.0} y={y:.0} gap={d:.0} {tags}");
                    prev = Some((*x, *y));
                }
            }
        }
        // `BOSS_WIN_AT=<frame>`: the boss dies then (the victory, its reward
        // cards); `REWARD_PICK=<1..3>@<frame>`: choose a card then.
        if let Some(at) = std::env::var("BOSS_WIN_AT").ok().and_then(|v| v.parse::<u64>().ok()) {
            if frames == at {
                if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                    let up = { let s = st.lock().unwrap(); s.boss_active && s.boss_spawned };
                    if up { crate::scenes::game::boss::arena::finish_boss(&mut canvas, &st); }
                    else { eprintln!("BOSS_WIN_AT: no boss up at frame {frames}"); }
                }
            }
        }
        if let Some((k, at)) = std::env::var("REWARD_PICK").ok().as_deref().and_then(|v| v.split_once('@'))
            .and_then(|(k, a)| Some((k.parse::<usize>().ok()?, a.parse::<u64>().ok()?))) {
            if frames == at {
                if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                    let ok = crate::scenes::game::perks::choose(&mut canvas, &st, k.saturating_sub(1));
                    eprintln!("REWARD_PICK {k}: {ok}");
                }
            }
        }

        // `HEADLESS_COLOSSUS_TORSO=1`: once the Colossus is up, kill its
        // hands, so the torso fights (its vent and meteor storm).
        if std::env::var("HEADLESS_COLOSSUS_TORSO").is_ok() {
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                let mut s = st.lock().unwrap();
                if s.boss_spawned && s.boss_kind == BossKind::Colossus && s.boss_parts.len() >= 4
                    && s.boss_parts[..2].iter().any(|p| p.alive) {
                    for p in s.boss_parts[..2].iter_mut() { p.alive = false; p.hp = 0; }
                }
            }
        }
        // `HEADLESS_COLOSSUS_HEAD=1`: once the Colossus is up, kill its hands
        // and torso, so the fight is at its last stage (the head alone).
        if std::env::var("HEADLESS_COLOSSUS_HEAD").is_ok() {
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                let mut s = st.lock().unwrap();
                if s.boss_spawned && s.boss_kind == BossKind::Colossus && s.boss_parts.len() >= 4
                    && s.boss_parts[..3].iter().any(|p| p.alive) {
                    for p in s.boss_parts[..3].iter_mut() { p.alive = false; p.hp = 0; }
                }
            }
        }
        // `HEADLESS_AB_AT=<frame>`: press E (fire the equipped ability) on
        // that frame, and log the player's velocity either side of it.
        if ab_at == Some(frames) || ab_at.map_or(false, |f| frames == f + 1) {
            let v = canvas.get_game_object("player").map(|o| o.momentum).unwrap_or((0.0, 0.0));
            eprintln!("ability-check: frame {frames} vel ({:.1}, {:.1}) bar {:.2} charges {}",
                v.0, v.1, canvas.get_f32("ab_bar"), canvas.get_i32("ab_charges"));
        }
        if ab_at == Some(frames) {
            let evt = KeyboardEvent { key: Key::Character("e".into()), state: KeyboardState::Pressed, modifiers: Modifiers::none() };
            OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(evt) as Box<dyn Event>);
            let v = canvas.get_game_object("player").map(|o| o.momentum).unwrap_or((0.0, 0.0));
            eprintln!("ability-check: fired, vel now ({:.1}, {:.1}) bar {:.2} charges {}",
                v.0, v.1, canvas.get_f32("ab_bar"), canvas.get_i32("ab_charges"));
        }
        // `HEADLESS_PHASE_CINE=<frame>`: press M (play the boss's phase
        // change) on that frame, to film it with HEADLESS_SHOT_DIR.
        if cine_at == Some(frames) {
            let evt = KeyboardEvent { key: Key::Character("m".into()), state: KeyboardState::Pressed, modifiers: Modifiers::none() };
            OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(evt) as Box<dyn Event>);
        }

        // Run one engine frame.
        OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);

        frames += 1;

        stage_camera(&mut canvas);
        // `HEADLESS_SPEED_METER=<frame>`: the FLYWHEEL clip's speed meter over
        // ordinary play, its TOP counting from that frame.
        if let Some(from) = std::env::var("HEADLESS_SPEED_METER").ok().and_then(|v| v.parse::<u64>().ok()) {
            static PEAK: std::sync::Mutex<(f32, f32)> = std::sync::Mutex::new((0.0, 0.0));
            let raw = canvas.get_game_object("player").map(|o| (o.momentum.0.powi(2) + o.momentum.1.powi(2)).sqrt()).unwrap_or(0.0);
            let mut p = PEAK.lock().unwrap();
            // A bounce pad lets one frame past the cap; that spike is not
            // the swing's speed, so a frame counts only as fast as the one
            // before it.
            let v = raw.min(p.1);
            p.1 = raw;
            if frames >= from { p.0 = p.0.max(v); }
            let peak = p.0;
            drop(p);
            speed_meter(&mut canvas, v, peak);
        }
        // `HEADLESS_CAM_BOSS=<zoom>`: film the boss, not the player — the
        // camera is put on the boss body after each tick (the game's own
        // camera moves it back during the next).
        if let Some(z) = std::env::var("HEADLESS_CAM_BOSS").ok().and_then(|v| v.parse::<f32>().ok()) {
            let at = canvas.get_game_object("boss").map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
            if let (Some((x, y)), Some(cam)) = (at, canvas.camera_mut()) {
                cam.zoom = z;
                cam.zoom_target = z;
                cam.position = (x - VW * 0.5 / z, y - VH * 0.5 / z);
            }
        }

        // Detect death BEFORE observing — the player object is removed on a
        // scene switch, so observe() would return None and mask the death.
        // Dying puts the death screen up over the run (the scene stays
        // "game"); its cause is published for exactly this.
        if canvas.get_bool("death_screen") {
            death_scene = Some(match canvas.get_var("death_cause") {
                Some(Value::Str(s)) => s,
                _ => "fall".to_string(),
            });
            let at = *died_at.get_or_insert(frames);
            // `HEADLESS_DEATH_KEY=space|escape`: press it as the hold ends
            // (RETRY / MENU), then run on and report where it went.
            if frames == at + death_hold {
                if let Ok(k) = std::env::var("HEADLESS_DEATH_KEY") {
                    let key = if k == "escape" { Key::Named(NamedKey::Escape) } else { Key::Named(NamedKey::Space) };
                    // Let go first: the auto-player may still be holding it,
                    // and a held key does not press again.
                    for state in [KeyboardState::Released, KeyboardState::Pressed] {
                        let evt = KeyboardEvent { key: key.clone(), state, modifiers: Modifiers::none() };
                        OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(evt) as Box<dyn Event>);
                    }
                    for i in 0..150u64 {
                        OnEvent::on_event(&mut canvas, &mut ctx, &sized, Box::new(TickEvent) as Box<dyn Event>);
                        if let Some(dir) = &shot_dir {
                            if i % 50 == 49 { shoot_frame(&mut canvas, dir, 900_000 + i); }
                        }
                    }
                    let left = ["dth_veil", "dth_title", "dth_retry_btn"].iter().filter(|n| canvas.get_game_object(n).is_some()).count();
                    let dead = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().map(|st| st.lock().unwrap().dead);
                    let mut lit: Vec<String> = Vec::new();
                    let mut ids = vec!["boss_light".to_string(), "__player_light".to_string()];
                    for i in 0..BOSS_BOLT_POOL_SIZE { ids.push(format!("boss_bolt_light_{i}")); }
                    for id in ids { if canvas.get_light(&id).map_or(false, |l| l.enabled) { lit.push(id); } }
                    eprintln!("death-key: lights still on {lit:?} ambient {:?}", canvas.ambient().map(|a| (a.color, a.strength)));
                    eprintln!("death-key: ability button visible {:?}", canvas.get_game_object("ab_btn").map(|o| o.visible));
                    eprintln!("death-key: scene {:?} screen {} overlay objects left {left} dead {dead:?} player visible {:?}",
                        canvas.active_scene(), canvas.get_bool("death_screen"),
                        canvas.get_game_object("player").map(|o| o.visible));
                }
            }
            if frames >= at + death_hold { break; }
            continue;
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
        //
        // Read the `boss_active` var, which `tick_boss` publishes every frame
        // for exactly this. The old test — "is the `boss` object visible" — is
        // blind to every MULTI-PART boss: for those the `boss` object is only an
        // anchor for the off-screen arrow and is deliberately hidden, the part
        // circles being the visible body. So the Colossus reported
        // `boss_entries=0` on every run of `--boss-stasis-down`, which is the
        // flag that exists to drive it, and the fight looked like it was never
        // starting when in fact it was never being LOOKED AT.
        if matches!(canvas.get_var("boss_active"), Some(Value::Bool(true))) {
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
        buff_desync_frames,
        airborne_frames,
        worst_starve_streak,
        flares_fired: get_i32_or(&canvas, "flares_fired", 0),
        flare_hearts_lost: get_i32_or(&canvas, "flare_hearts_lost", 0),
        flare_saves: get_i32_or(&canvas, "flare_saves", 0),
        flares_without_shelter,
        shelter_dx_sum,
        shelter_dx_worst,
        worst_frontier_overshoot: worst_overshoot,
        frontier_repairs: get_i32_or(&canvas, "frontier_repairs", 0),
        census,
    }
}

/// Run several episodes (each boots a fresh canvas) and aggregate.
pub fn run(episodes: u64, max_frames: u64, boss_mode: bool, force_fall: bool, boss_warp: bool, weakpoint_check: bool, stasis_down: bool, flare_test: bool, shelter_check: bool, start_minute: f32) -> AggregateReport {
    let mut agg = AggregateReport::default();
    let mut episode_idx = 0u64;
    while episode_idx < episodes {
        let ep = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_episode(max_frames, boss_mode, force_fall, boss_warp, weakpoint_check, stasis_down, flare_test, shelter_check, start_minute)
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
                buff_desync_frames: 0,
                airborne_frames: 0,
                worst_starve_streak: 0,
                flares_fired: 0,
                flare_hearts_lost: 0,
                flare_saves: 0,
                flares_without_shelter: 0,
                shelter_dx_sum: 0.0,
                shelter_dx_worst: 0.0,
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
        agg.buff_desync_frames += ep.buff_desync_frames;
        agg.airborne_frames += ep.airborne_frames;
        agg.worst_starve_streak = agg.worst_starve_streak.max(ep.worst_starve_streak);
        agg.flares_fired += ep.flares_fired;
        agg.flare_hearts_lost += ep.flare_hearts_lost;
        agg.flare_saves += ep.flare_saves;
        agg.flares_without_shelter += ep.flares_without_shelter;
        agg.shelter_dx_sum += ep.shelter_dx_sum;
        agg.shelter_dx_worst = agg.shelter_dx_worst.max(ep.shelter_dx_worst);
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
