#![allow(dead_code)]
// ── Virtual resolution ────────────────────────────────────────────────────────
pub const VW: f32 = 3840.0;
pub const VH: f32 = 2160.0;

// ── Physics ───────────────────────────────────────────────────────────────────
pub const GRAVITY:        f32 = 0.82;
pub const SWING_TENSION:  f32 = 1.06;
pub const MOMENTUM_CAP:   f32 = 50.0;
pub const ROPE_LEN_MIN:   f32 = 200.0;
pub const ROPE_LEN_MAX:   f32 = 720.0;
pub const SWING_DRAG:     f32 = 0.999;
pub const GRAB_SURGE:     f32 = 4.2;
pub const GRAB_TANGENT_SURGE_SCALE: f32 = 0.12;
pub const GRAB_TANGENT_SURGE_MAX:   f32 = 4.0;
pub const GRAB_SURGE_MULT: f32 = 2.6;
pub const GRAB_VERTICAL_BOOST: f32 = 1.28;
pub const GRAB_SPIN_DISABLE_SPEED: f32 = 50.0;
pub const SPECIAL_HOOK_BOOST_SURGE: f32 = 84.0;
pub const SPECIAL_HOOK_VERTICAL_BOOST: f32 = 1.18;
pub const SPECIAL_HOOK_MIN_SPEED: f32 = 118.0;
pub const SPECIAL_HOOK_MOMENTUM_CAP: f32 = 74.0;
pub const SPECIAL_HOOK_CAP_WINDOW_TICKS: i32 = 84;
pub const RELEASE_MIN_SWING_SPEED: f32 = 3.2;
pub const RELEASE_SURGE_SCALE: f32 = 0.42;
pub const RELEASE_SURGE_MAX: f32 = 14.0;
pub const RELEASE_VERTICAL_BOOST: f32 = 1.50;

// ── Object sizes ──────────────────────────────────────────────────────────────
pub const PLAYER_R:       f32 = 58.0;
pub const HOOK_R:         f32 = 38.0;
/// Display/collision radius for artifact-mode grab hooks (1.5× regular hook).
pub const HOOK_ARTIFACT_R: f32 = HOOK_R * 1.5;
pub const ROPE_THICKNESS: f32 = 60.0;
pub const AIRSHIELD_W:    f32 = 220.0;
pub const AIRSHIELD_H:    f32 = 160.0;
pub const AIRSHIELD_SPEED_THRESHOLD: f32 = 30.0;
pub const AIRSHIELD_AHEAD_OFFSET:    f32 = 110.0;
pub const AIRSHIELD_ANIM_FPS:        f32 = 16.0;

// ── Generation — General ──────────────────────────────────────────────────────

/// How far ahead of the player world objects are pre-generated (px).
/// Increase → more objects buffered ahead (smoother at high speed, more memory).
/// Decrease → objects may pop in visibly when moving fast.
pub const GEN_AHEAD:      f32 = VW * 3.5;

/// Max hooks the generator places per game tick (frame).
/// Higher = faster queue fill but more CPU per frame.
pub const HOOKS_SPAWN_BUDGET_PER_TICK:    usize = 20;
pub const PADS_SPAWN_BUDGET_PER_TICK:     usize = 2;
pub const SPINNERS_SPAWN_BUDGET_PER_TICK: usize = 2;
pub const FLIPS_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const ZERO_G_SPAWN_BUDGET_PER_TICK:   usize = 1;
pub const GATES_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const COIN_BATCHES_BUDGET_PER_TICK:   usize = 1;

// ── Generation — Grab Points (Hooks) ─────────────────────────────────────────

/// How many hooks the pending queue is filled to per batch call.
/// Increase → longer lookahead, smoother streaming.
pub const MAX_HOOKS_LIVE: usize = 40;

/// Object pool size. Must be ≥ (MAX_HOOKS_LIVE + starter hooks).
/// Increasing this is safe; decreasing below ~20 will cause pool starvation.
pub const HOOK_POOL_SIZE: usize = 68;

/// How long the player must HOLD space/mouse at the start prompt before the run
/// begins (ticks; 90 = 1.5 s at 60 fps). Prevents a stray/retry click from
/// instantly launching + grabbing.
pub const START_HOLD_TICKS: i32 = 90;

/// World Y bounds for grab points.
/// HOOK_Y_MIN is the top of the playable zone (negative = above the horizon).
/// HOOK_Y_MAX is the bottom of the playable zone.
/// Narrowing this range makes hooks appear in a tighter band.
pub const HOOK_Y_MIN:      f32 = -200.0;
pub const HOOK_Y_MAX:      f32 = 750.0;

/// Unused by the feature generator (retained for API compatibility).
pub const HOOK_BATCH_MIN_Y_GAP: f32 = 80.0;

/// When placing a new hook, any previously placed hook within this vertical
/// distance is rejected (bottom hook discarded, top hook kept).
/// Increase to force more Y separation between consecutive hooks.
/// Set to 0.0 to disable the anti-stacking check entirely.
pub const HOOK_CLOSE_Y_THRESHOLD: f32 = 220.0;

// ── Generation — Rope Reach Rules ────────────────────────────────────────────
//
// A hop is legal when the next grab node sits inside an ELLIPSE centred on the
// previous one:
//
//     (dx / HOP_REACH_X)^2 + (dy / HOP_REACH_UP or HOP_REACH_DOWN)^2 <= 1
//
// An ellipse rather than a circle because the two directions are not equally
// expensive: dropping to a lower node is nearly free (gravity does the work and
// the player only has to be within a rope length by the time they arrive),
// while climbing to a higher one has to be paid for out of swing momentum. So
// the downward radius is larger than the upward one, which buys real vertical
// variety without ever producing a hop the player cannot make.
//
// THIS ENVELOPE IS ENFORCED TWICE, and it has to be:
//   1. `level_gen::generate_next_hook` proposes a hop inside it, and
//   2. `spawning::spawn_hooks` clamps the FINAL position back inside it after
//      its hazard-avoidance passes have moved the node in Y.
// Step 2 is not redundant. The avoidance passes move a node by up to 620 px
// with no reference to the previous node at all, so a correctly generated hop
// routinely landed outside reach. Measured on the pre-fix build: 27% of
// consecutive node pairs were farther apart than a full rope length.
//
// `sim_tests::hook_generation_stays_reachable_and_bounded` guards (1);
// `sim_tests::hop_envelope_is_symmetric_about_the_previous_node` guards the
// geometry helper both of them share.

/// Hard minimum Euclidean distance between consecutive grab nodes.
/// Closer than this the two nodes read as one blob rather than a hop.
pub const HOOK_MIN_REACH: f32 = ROPE_LEN_MAX * 0.5; // 360.0

/// Horizontal semi-axis of the hop envelope. One rope length: the player can
/// always be at the front of their arc when they release.
pub const HOP_REACH_X: f32 = ROPE_LEN_MAX; // 720.0

/// Upward semi-axis. Deliberately under a rope length — climbing costs momentum.
pub const HOP_REACH_UP: f32 = ROPE_LEN_MAX * 0.85; // 612.0

/// Downward semi-axis. Over a rope length is safe: the player falls into range.
pub const HOP_REACH_DOWN: f32 = ROPE_LEN_MAX * 1.15; // 828.0

/// The strictest single radius a hop is measured against, kept for callers that
/// want one number rather than the ellipse.
pub const HOOK_MAX_REACH: f32 = ROPE_LEN_MAX; // 720.0

/// Horizontal stride per hop as a FRACTION of `HOP_REACH_X`, at the two ends of
/// the difficulty curve. Short strides early mean quick, forgiving chains; long
/// strides late mean committing to most of a rope length every time. Never
/// above 1.0, so the envelope is never left.
pub const HOOK_STRIDE_FRAC_EASY_MIN: f32 = 0.58; // ~418 px
pub const HOOK_STRIDE_FRAC_EASY_MAX: f32 = 0.72; // ~518 px
pub const HOOK_STRIDE_FRAC_HARD_MIN: f32 = 0.80; // ~576 px
pub const HOOK_STRIDE_FRAC_HARD_MAX: f32 = 0.97; // ~698 px

/// Fraction of the available vertical budget a hop may spend, at the two ends
/// of the curve. Early hops stay near the previous node's height; late hops use
/// nearly the whole cone, so the line of nodes climbs and dives.
pub const HOOK_VERT_FRAC_EASY: f32 = 0.45;
pub const HOOK_VERT_FRAC_HARD: f32 = 1.00;

/// Safety margin applied when the spawner clamps a hazard-avoided node back
/// into the envelope, so float error can never leave it exactly on the edge.
pub const HOP_REACH_MARGIN: f32 = 0.97;

// ── Generation — Bounce Pads ──────────────────────────────────────────────────

pub const PAD_POOL_SIZE:  usize = 32;

/// X gap between consecutive bounce pads (px), AT THE START OF A RUN.
///
/// Pads are the anti-fall net, so the opening is deliberately generous — one
/// roughly every four seconds — and `hazards::Support::Pad` widens these gaps
/// as the run goes on, bottoming out around 6 700–11 500 px. Change these to
/// move the early game; change the support floor to move the late game.
pub const PAD_GAP_MIN:    f32 = 3000.0;
pub const PAD_GAP_MAX:    f32 = 5200.0;

// techbouncernew.gif is decoded into this fixed gameplay footprint.
// Art scaling changes should happen in the loader, not by changing pad geometry.
pub const PAD_W:          f32 = 775.0;
pub const PAD_H:          f32 = 262.5;
/// techbouncernew.gif fills the full frame — collision covers the entire width.
pub const PAD_COLLISION_WIDTH_FACTOR: f32 = 1.0;

#[inline]
pub fn pad_collision_w() -> f32 {
    PAD_W * PAD_COLLISION_WIDTH_FACTOR
}

#[inline]
pub fn pad_collision_left(pad_left: f32) -> f32 {
    pad_left + (PAD_W - pad_collision_w()) * 0.5
}

/// How close (px) in X a pad must be to a hook before the Y floor is applied.
pub const PAD_HOOK_NEAR_X:      f32 = 2200.0;

/// Minimum Y clearance below a nearby hook before a pad is allowed.
/// Increase to push pads further below hooks.
pub const PAD_BELOW_HOOK_Y_GAP: f32 = 400.0;

/// Hard world Y floor for pad spawning. Pads never appear above this.
/// Set to HOOK_Y_MAX + N to keep pads visually below all grab points.
pub const PAD_Y_MIN: f32 = HOOK_Y_MAX + 150.0; // ≈ 1200.0

/// Fixed upward velocity applied when the player hits a bounce pad.
pub const PAD_BOUNCE_VY: f32 = -88.0;

/// Restitution (bounciness) applied when a space asteroid hits a bounce pad.
pub const PAD_ASTEROID_RESTITUTION: f32 = 0.62;

/// How far a moving pad travels from its origin (px). 0 = static.
pub const PAD_MOVE_RANGE: f32 = 250.0;
/// Speed of pad oscillation (px/tick).
pub const PAD_MOVE_SPEED: f32 = 3.0;

pub fn pad_corner_radius() -> f32 {
    // techbouncernew.gif has a pill/capsule shape — corner radius ≈ half height.
    (PAD_H * 0.45).clamp(1.0, PAD_H * 0.5 - 1.0)
}

// ── Generation — Spinners ─────────────────────────────────────────────────────

pub const SPINNER_POOL_SIZE: usize = 14;

/// X gap between consecutive spinners (px).
/// Increase both to make spinners rarer.
pub const SPINNER_GAP_MIN:   f32 = 7000.0;
pub const SPINNER_GAP_MAX:   f32 = 11000.0;

pub const SPINNER_W:         f32 = 620.0;
pub const SPINNER_H:         f32 = 70.0;
/// Base rotation speed (deg/tick). Scaled per zone in level_gen.rs.
pub const SPINNER_ROT_SPEED: f32 = 6.4;

/// A hook is only considered for spinner Y-relocation if it falls within this
/// horizontal distance of the spinner's centre. (Half spinner width = 310 px.)
/// Set lower to reduce spinner influence on hook placement.
/// Legacy X-only proximity threshold (superseded by HOOK_SPINNER_PROX_R).
pub const HOOK_SPINNER_MIN_X_GAP: f32 = 200.0;
pub const HOOK_SPINNER_PUSH_X:    f32 = 300.0;

/// Euclidean proximity radius for the spinner-avoidance check.
/// = SPINNER_W / 2 × 1.5 (one and a half spinner half-widths).
/// Any grab node within this distance of a spinner centre is relocated.
pub const HOOK_SPINNER_PROX_R: f32 = SPINNER_W * 0.75; // 465.0

/// How far (px) above a spinner centre a relocated hook is placed.
/// Always pushes upward (never below) to keep grabs clear of the hazard.
pub const HOOK_SPINNER_Y_OFFSET:  f32 = 950.0;

/// How far (px) a grab node is pushed above a bounce pad's top edge when
/// it lands too close to one.
pub const HOOK_PAD_CLEAR_Y: f32 = 800.0;

/// Zone multipliers for spinner rotation speed.
pub const SPINNER_BLACK_MOVE_AMP_MIN: f32 = 120.0;
pub const SPINNER_BLACK_MOVE_AMP_MAX: f32 = 260.0;
pub const SPINNER_BLACK_MOVE_SPEED_MIN: f32 = 1.1;
pub const SPINNER_BLACK_MOVE_SPEED_MAX: f32 = 2.1;

// ── Generation — Zones ────────────────────────────────────────────────────────

// The zone step moved to `difficulty::ZONE_CYCLE_DISTANCE`, which is authored
// in minutes of play rather than raw pixels — 20 000 px is under 30 seconds, so
// the "→ repeat" this comment promised was reached almost immediately and then
// clamped away by a `.min(2)` that made it never repeat at all.

/// Spinner speed multipliers per zone. BLACK_ZONE > PURPLE_ZONE > START_ZONE.
pub const START_ZONE_SPINNER_MULT:f32 = 0.50;
pub const PURPLE_ZONE_SPINNER_MULT:f32 = 1.00;
pub const BLACK_ZONE_SPINNER_MULT:f32 = 1.50;

pub const SPINNER_HIT_PUSH_X:f32 = 11.0;
pub const SPINNER_HIT_PUSH_Y:f32 = -28.0;

// ── Generation — Coins ────────────────────────────────────────────────────────

pub const COIN_POOL_SIZE:    usize = 30;

/// X gap between coin spawns (px). Narrower = more coins.
pub const COIN_GAP_MIN:      f32 = 2200.0;
pub const COIN_GAP_MAX:      f32 = 4200.0;

pub const COIN_R:            f32 = 48.0;
pub const COIN_SCORE:        u32 = 125;
pub const COIN_ARRAY_COUNT:  usize = 5;
pub const COIN_ARRAY_SPACING:f32 = 120.0;
pub const COIN_CURVE_RISE:   f32 = 60.0;
/// Probability (0–1) that a coin spawn is an array rather than single coin.
pub const COIN_ARRAY_CHANCE: f32 = 0.28;
pub const COIN_ARRAY_HOOK_DX:f32 = 600.0;
pub const COIN_ARRAY_HOOK_DY:f32 = -1200.0; // much higher above anchor hook
pub const COIN_ARRAY_Y_MIN:  f32 = -950.0;  // pushed high above hook zone
pub const COIN_ARRAY_Y_MAX:  f32 = -380.0;  // coins always above highest hooks
pub const COIN_SINGLE_Y_MIN: f32 = -750.0;
pub const COIN_SINGLE_Y_MAX: f32 = 380.0;
/// 3×3 grid coin pattern.
pub const COIN_GRID_COLS:      usize = 3;
pub const COIN_GRID_ROWS:      usize = 3;
pub const COIN_GRID_SPACING_X: f32   = 120.0;
pub const COIN_GRID_SPACING_Y: f32   = 120.0;
/// Probability (0–1) that a coin spawn is a 3×3 grid.
pub const COIN_GRID_CHANCE:    f32   = 0.30;
/// Coin cross formation: center with four sides.
pub const COIN_CROSS_COUNT:     usize = 5;
pub const COIN_CROSS_SPACING:   f32   = 140.0;
pub const COIN_CROSS_CHANCE:    f32   = 0.18;
/// Coin diamond formation: large diamond shape around a center point.
pub const COIN_DIAMOND_COUNT:   usize = 9;
pub const COIN_DIAMOND_SPACING: f32   = 100.0;
pub const COIN_DIAMOND_CHANCE:  f32   = 0.12;
/// Radius of the coin magnet pickup effect (px).
pub const COIN_MAGNET_RADIUS:f32 = 180.0;
pub const COIN_MAGNET_PULL:  f32 = 0.37;

// ── Generation — Flip Pickups ─────────────────────────────────────────────────

pub const FLIP_POOL_SIZE:    usize = 16;
/// X gap between gravity-flip pickups (px). Increase = rarer flips.
pub const FLIP_GAP_MIN:      f32 = 14000.0;
pub const FLIP_GAP_MAX:      f32 = 24000.0;
pub const FLIP_W:            f32 = 480.0;
pub const FLIP_H:            f32 = 480.0;
/// Decode size for the space_rip animation — kept small so GPU upscales cheaply.
pub const FLIP_ANIM_W:       f32 = 160.0;
pub const FLIP_ANIM_H:       f32 = 160.0;
/// How long a gravity flip lasts (ticks). 300 = 5 s at 60 fps.
pub const FLIP_DURATION:     u32 = 300;

// ── Generation — Score ×2 Pickups ────────────────────────────────────────────

pub const SCORE_X2_POOL_SIZE: usize = 16;
/// X gap between score-doubler pickups (px).
pub const SCORE_X2_GAP_MIN:   f32 = 12000.0;
pub const SCORE_X2_GAP_MAX:   f32 = 20000.0;
pub const SCORE_X2_W:         f32 = 160.0;
pub const SCORE_X2_H:         f32 = 160.0;
/// How long score×2 lasts (ticks). 600 = 10 s at 60 fps.
pub const SCORE_X2_DURATION:  u32 = 600;

// ── Generation — Zero-G Pickups ───────────────────────────────────────────────

pub const ZERO_G_POOL_SIZE:   usize = 14;
/// X gap between zero-gravity pickups (px).
pub const ZERO_G_GAP_MIN:     f32 = 13000.0;
pub const ZERO_G_GAP_MAX:     f32 = 22000.0;
pub const ZERO_G_W:           f32 = 120.0;
pub const ZERO_G_H:           f32 = 120.0;
/// How long zero-G lasts (ticks). 480 = 8 s at 60 fps.
pub const ZERO_G_DURATION:    u32 = 480;
/// Fraction of normal gravity applied during zero-G (0 = weightless, 1 = full).
pub const ZERO_G_GRAVITY_SCALE: f32 = 0.55;

// ── Generation — Gates ────────────────────────────────────────────────────────

pub const GATE_POOL_SIZE:    usize = 10;
pub const GATE_GAP_MIN:      f32 = 7600.0;
pub const GATE_GAP_MAX:      f32 = 12000.0;
pub const GATE_W:            f32 = 190.0;
pub const GATE_GAP_H:        f32 = 560.0;
pub const GATE_MIN_CLUSTER_SEPARATION: f32 = 10000.0;
pub const GATE_VERTICAL_OVERFLOW: f32 = 700.0;
pub const GATES_ENABLED:     bool = false;
pub const GATE_TOP_BASE_H:   f32 = (VH - GATE_GAP_H) * (2.0 / 3.0);
pub const GATE_BOT_BASE_H:   f32 = (VH - GATE_GAP_H) - GATE_TOP_BASE_H;
pub const GATE_TOP_SEG_H:    f32 = GATE_TOP_BASE_H + GATE_VERTICAL_OVERFLOW;
pub const GATE_BOT_SEG_H:    f32 = GATE_BOT_BASE_H + GATE_VERTICAL_OVERFLOW;

// ── Dev / Testing ─────────────────────────────────────────────────────────────

/// Set to true to force the test lane layout for visual inspection.
pub const TEST_LAYOUT_MODE: bool = false;
pub const TEST_HOOK_GAP: f32 = 760.0;

// ── Zoom ──────────────────────────────────────────────────────────────────────
pub const ZOOM_TOP_MARGIN:  f32 = VH * 0.14;
pub const ZOOM_MAX:         f32 = 3.2;
pub const ZOOM_OUT_LERP:    f32 = 0.10;
pub const ZOOM_IN_LERP:     f32 = 0.02;
pub const ZOOM_LOOKAHEAD_T: f32 = 12.0;

// ── Colours ───────────────────────────────────────────────────────────────────
pub const C_SKY_TOP:  (u8,u8,u8) = (15,  20,  45 );
pub const C_SKY_BOT:  (u8,u8,u8) = (30,  50,  90 );
pub const C_ZONE_PURPLE_TOP:(u8,u8,u8) = (42,  16,  70 );
pub const C_ZONE_PURPLE_BOT:(u8,u8,u8) = (88,  36, 128 );
pub const C_ZONE_BLACK_TOP: (u8,u8,u8) = (220, 130, 35);
pub const C_ZONE_BLACK_BOT: (u8,u8,u8) = (255, 175, 80);
pub const C_PLAYER:   (u8,u8,u8) = (80,  220, 160);

/// Player character colour palette — shared by shop.rs and build_scene.rs.
/// Index 0 is the animated calicoball; the colour here is just for the shop card preview.
pub const PLAYER_CHAR_COLORS: &[(u8, u8, u8)] = &[
    (255, 195, 140), // 0 calico (animated cat ball — colour shown on shop card)
    (200, 200, 220), // 1 silver
    ( 60, 160, 240), // 2 blue
    ( 80, 210, 130), // 3 green
    (240, 150,  60), // 4 orange
    (180, 100, 240), // 5 purple
    (240,  90,  90), // 6 red
];
pub const PLAYER_CHAR_NAMES: &[&str] = &["CALICO", "SILVER", "BLUE", "GREEN", "ORANGE", "PURPLE", "RED"];

pub const C_HOOK:     (u8,u8,u8) = (200, 60,  20 );
pub const C_HOOK_ON:  (u8,u8,u8) = (255, 90,  70 );
pub const C_HOOK_NEAR:(u8,u8,u8) = (255, 120, 50 );
pub const C_HOOK_SPECIAL:      (u8,u8,u8) = (52, 196, 84);
pub const C_HOOK_SPECIAL_NEAR: (u8,u8,u8) = (105, 244, 140);
pub const C_HOOK_SPECIAL_ON:   (u8,u8,u8) = (175, 255, 196);
pub const C_ROPE:     (u8,u8,u8) = (220, 220, 220);
pub const C_DANGER:   (u8,u8,u8) = (200, 50,  50 );
pub const C_PAD:      (u8,u8,u8) = (60,  200, 255);
pub const C_PAD_HIT:  (u8,u8,u8) = (160, 255, 255);
pub const C_SPINNER:  (u8,u8,u8) = (255, 100, 95);
pub const C_COIN:     (u8,u8,u8) = (255, 95, 210);
pub const C_FLIP:     (u8,u8,u8) = (255, 245, 120);

// Zone-specific object palettes (zone 0 keeps existing base colors).
pub const C_HOOK_ZONE1:      (u8,u8,u8) = (90, 230, 210);
pub const C_HOOK_NEAR_ZONE1: (u8,u8,u8) = (140, 255, 235);
pub const C_HOOK_ON_ZONE1:   (u8,u8,u8) = (210, 255, 245);
pub const C_PAD_ZONE1:       (u8,u8,u8) = (102, 74, 170);
pub const C_PAD_HIT_ZONE1:   (u8,u8,u8) = (150, 120, 220);
pub const C_SPINNER_ZONE1:   (u8,u8,u8) = (200, 128, 255);

pub const C_HOOK_ZONE2:      (u8,u8,u8) = (106, 78, 210);
pub const C_HOOK_NEAR_ZONE2: (u8,u8,u8) = (156, 126, 250);
pub const C_HOOK_ON_ZONE2:   (u8,u8,u8) = (214, 194, 255);
pub const C_PAD_ZONE2:       (u8,u8,u8) = (210, 126, 46);
pub const C_PAD_HIT_ZONE2:   (u8,u8,u8) = (255, 170, 92);
pub const C_SPINNER_ZONE2:   (u8,u8,u8) = (255, 193, 88);

// ── Spawn positions ───────────────────────────────────────────────────────────
pub const SPAWN_X: f32 = VW * 0.22;
pub const SPAWN_Y: f32 = VH * 0.38;
pub const START_HOOK_X: f32 = SPAWN_X + 160.0;
pub const START_HOOK_Y: f32 = SPAWN_Y - 420.0;

// ── Asset bytes ──────────────────────────────────────────────────────────────
pub const ASSET_COIN_GIF: &[u8] = include_bytes!("../assets/coin.gif");
pub const ASSET_SCORE_X2_GIF: &[u8] = include_bytes!("../assets/2x.gif");
pub const ASSET_TECH_BOUNCE_GIF: &[u8] = include_bytes!("../assets/techbouncernew.gif");
pub const TECH_BOUNCE_FPS: f32 = 12.0;
pub const ASSET_BGM_TRACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/synful_reach.mp3");
pub const ASSET_SWOOSH_SFX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/swipe.mp3");
pub const ASSET_COIN_SFX_1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/coin_collect.mp3");
pub const ASSET_COIN_SFX_2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/coin_up.mp3");
pub const ASSET_COIN_SFX_3: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/coin_bling.mp3");
pub const ASSET_COIN_SFX_4: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/coin_ambience.mp3");
pub const ASSET_BGM_TRACK_1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/music_1.mp3");
pub const ASSET_BGM_TRACK_2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/music_2.mp3");
pub const ASSET_BGM_TRACK_3: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/music_3.mp3");
pub const ASSET_MENU_BGM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Roses_new.mp3");
pub const ASSET_MENU_BGM_2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Pill.mp3");
pub const ASSET_MENU_BGM_3: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Menumusic.mp3");
pub const ASSET_BACKGROUND: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/background.png");
pub const ASSET_BACKGROUND_2: &[u8] = include_bytes!("../assets/background_2.webp");
pub const ASSET_AURORA_EARTH_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/aurora_earth.gif");
pub const ASSET_MAN_GAME_OVER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/man_game_over.mp3");
pub const ASSET_ARCADE_GAME_OVER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/arcade_game_over.mp3");
pub const ASSET_WOBBLY_MEOW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/wobbly_meow.mp3");
pub const ASSET_CARTOON_CAT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cartoon_cat.mp3");
pub const ASSET_ASTEROID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/asteroid.webp");
pub const ASSET_HOOK_ARTIFACT_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hook_artifact.gif");
pub const ASSET_HOOK_ARTIFACT_GREEN_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hook_artifact_green.gif");
/// Average ticks between automatic comet spawn attempts (at 60 fps ≈ 5 seconds).
pub const COMET_SPAWN_INTERVAL: u32 = 300;
/// Ticks each successive comet in a back-to-back burst is advanced by, so a
/// wave arrives in sequence instead of as one wide wall. A wall has no gap to
/// swing through, which is the difference between harder and unfair.
pub const COMET_BURST_STAGGER: u32 = 26;
pub const HOOK_ARTIFACT_FPS: f32 = 13.0;
pub const HOOK_ARTIFACT_INTRO_FPS: f32 = 24.0;
pub const ASSET_THRUSTER1_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/thruster1.gif");
pub const ASSET_CALICOBALL_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/calicoball.gif");
pub const ASSET_BLACKHOLE1_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/blackhole1.gif");
pub const ASSET_WORMHOLE2_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/wormhole2.gif");
pub const ASSET_GWELLON_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gwellon.gif");
pub const ASSET_GWELLOFF_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gwelloff.gif");
pub const ASSET_ZERO_G_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/ZeroG.gif");
pub const ASSET_SPACE_RIP_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/space_rip.gif");
pub const CALICO_FPS: f32 = 12.0;
pub const GWELL_FPS: f32 = 10.0;
pub const BLACKHOLE_FPS: f32 = 12.0;
pub const PAD_THRUSTER_FPS: f32 = 12.0;
pub const PAD_THRUSTER_W: f32 = PAD_W * 0.24;
pub const PAD_THRUSTER_H: f32 = PAD_H * 0.775;
// Extra top pixels of the thruster image tucked inside the pad body.
pub const PAD_THRUSTER_HIDE_TOP: f32 = 70.0;
// Small additional embed so thruster art top blends into the pad underside.
pub const PAD_THRUSTER_RAISE_Y: f32 = PAD_THRUSTER_H * 0.05;

// ── Generation — Gravity Wells ────────────────────────────────────────────────

pub const GWELL_POOL_SIZE:     usize = 10;

/// X gap between consecutive gravity wells (px).
/// Increase both to make wells rarer. Decrease for more aggressive well density.
pub const GWELL_GAP_MIN:       f32 = 9000.0;
pub const GWELL_GAP_MAX:       f32 = 15000.0;

/// Pull radius range (px). Larger = well affects a wider area.
/// Min is reached for easy wells; max for hard wells.
pub const GWELL_RADIUS_MIN:    f32 = 540.0;
pub const GWELL_RADIUS_MAX:    f32 = 1080.0;

/// Pull force range. 0 = no pull, 1 = full gravity override.
/// Increase GWELL_STRENGTH_MAX to make wells harder to escape.
pub const GWELL_STRENGTH_MIN:  f32 = 0.9;
pub const GWELL_STRENGTH_MAX:  f32 = 1.2;

/// How long the well is active before going dormant (ticks). 240 = 4 s @ 60 fps.
pub const GWELL_ON_TICKS:      u32 = 240;
/// How long the well stays dormant before reactivating (ticks). 180 = 3 s @ 60 fps.
pub const GWELL_OFF_TICKS:     u32 = 180;

/// World Y range for well spawning. Expressed as a fraction of VH.
/// Adjust these to keep wells away from the very top or bottom of the screen.
pub const GWELL_Y_MIN:         f32 = VH * 0.15;
pub const GWELL_Y_MAX:         f32 = VH * 0.80;

pub const GWELL_SPAWN_BUDGET:  usize = 1;

/// Visual ring scale relative to player diameter.
/// 3× = smallest well looks 3× the player. 10× = largest looks much bigger.
pub const GWELL_VISUAL_SCALE_MIN: f32 = 3.0;
pub const GWELL_VISUAL_SCALE_MAX: f32 = 10.0;
/// Number of concentric alpha rings rendered per well. More = richer visual.
pub const GWELL_RING_COUNT:    u32 = 5;
pub const GWELL_PULSE_MIN:     f32 = 0.7;
pub const GWELL_PULSE_SPEED:   f32 = 0.08;
/// The rope disconnects from a grab point when the player enters this fraction
/// of the well's radius. 0.5 = disconnect at half-radius.
pub const GWELL_DISCONNECT_FRAC: f32 = 0.5;
pub const C_GWELL_ACTIVE:      (u8,u8,u8) = (130, 80, 255);
pub const C_GWELL_DORMANT:     (u8,u8,u8) = (60, 40, 110);

// ── Turrets ───────────────────────────────────────────────────────────────────
pub const TURRET_POOL_SIZE:      usize = 8;
pub const TURRET_R:              f32 = 50.0;
pub const TURRET_BARREL_LEN:    f32 = 50.0;
pub const TURRET_BARREL_W:      f32 = 20.0;
pub const TURRET_FULL_SIZE:     f32 = (TURRET_R + TURRET_BARREL_LEN) * 2.0;
pub const TURRET_GAP_MIN:       f32 = 7000.0;
pub const TURRET_GAP_MAX:       f32 = 12000.0;
pub const TURRET_SHOOT_INTERVAL:u32 = 180;  // 3 seconds at 60fps
pub const TURRET_SHOOT_INTERVAL_FAST:u32 = 150; // phase 1 interval
pub const TURRET_SHOOT_INTERVAL_P2:  u32 = 130; // phase 2+ interval (slightly faster than phase 1)
pub const TURRET_SUCCESSIVE_GAP:     f32 = 260.0; // px between successive phase-2 shots along fire axis
pub const TURRET_SPAWN_BUDGET:  usize = 1;
pub const TURRET_Y_MIN:         f32 = VH * 0.12;
pub const TURRET_Y_MAX:         f32 = VH * 0.80;
pub const TURRET_DETECT_RADIUS: f32 = 2800.0;
pub const TURRET_PHASE_2_X:     f32 = 20_000.0;
pub const TURRET_PHASE_3_X:     f32 = 40_000.0;
pub const TURRET_DUAL_SHOT_GAP: f32 = 44.0;   // kept for reference, no longer used for parallel
pub const TURRET_PREDICT_MAX_T: f32 = 60.0;   // max lead-time clamp (ticks); raised for better phase-3 aim
pub const BULLET_POOL_SIZE:     usize = 64;

// ── Boss fight ────────────────────────────────────────────────────────────────
/// Legacy single-fight trigger. Superseded by `mode::boss_trigger_distance`,
/// which schedules several fights per run; kept only as the debug-warp default.
pub const BOSS_THRESHOLD_X:      f32   = 20_000.0;
/// Empty space between consecutive boss arenas.
/// (Where the arenas START is `mode::BOSS_ARENA_ORIGIN_X`, which is derived
/// from the difficulty curve so it cannot drift into the reachable level.)
pub const BOSS_ARENA_GAP:        f32   = 40_000.0;
pub const BOSS_ZONE_X1:          f32   = 20_000.0; // left wall of boss arena
pub const BOSS_ZONE_X2:          f32   = 34_000.0; // right wall of boss arena (doubled)
pub const BOSS_ENTRY_DELAY_TICKS: u32  = 180;      // 3 seconds before boss appears
pub const BOSS_SIZE:             f32   = 360.0;    // width and height of boss body
pub const BOSS_MAX_HP:           i32   = 20;
pub const BOSS_BOLT_POOL_SIZE:   usize = 24;
pub const BOSS_BOLT_W:           f32   = 80.0;
pub const BOSS_BOLT_H:           f32   = 30.0;
pub const BOSS_BOLT_SPEED:       f32   = 16.0;
pub const BOSS_BOLT_LIFETIME:    u32   = 360;      // 6 s at 60 fps
pub const BOSS_SHOOT_INTERVAL:   u32   = 90;       // 1.5 s at 60 fps
pub const BOSS_FLOAT_SPEED:      f32   = 2.8;      // kept for reference
pub const BOSS_HP_BAR_W:         f32   = 900.0;
pub const BOSS_HP_BAR_H:         f32   = 50.0;
/// Gravity multiplier applied to the player while inside the boss arena.
pub const BOSS_GRAVITY_SCALE:    f32   = 0.05;
/// Number of decorative asteroid GIFs placed around the boss arena.
pub const BOSS_ASTEROID_COUNT:   usize = 16; // 4 cols × 4 rows
/// Radius within which a hit counts as on a weakpoint.
pub const BOSS_WEAKPOINT_R:      f32   = 130.0;
/// Weakpoint offsets (from boss centre): top, right, bottom, left.
/// A buffed hit near one of these damages the boss.
pub const BOSS_WEAKPOINT_OFFSETS: [(f32, f32); 4] = [
    (0.0,           -BOSS_SIZE * 0.36),
    ( BOSS_SIZE * 0.36, 0.0),
    (0.0,            BOSS_SIZE * 0.36),
    (-BOSS_SIZE * 0.36, 0.0),
];
/// Ticks between boss darkness attacks.
pub const BOSS_DARK_INTERVAL:    u32   = 600;  // 10 s
/// Ticks a darkness phase lasts.
pub const BOSS_DARK_DURATION:    u32   = 180;  // 3 s
/// Ticks of warning before darkness strikes.
pub const BOSS_DARK_TELEGRAPH:   u32   = 60;   // 1 s

// ── Last-boss barrier / generators / bait-and-bail ───────────────────────────
/// How many generators power the protective barrier.
pub const BOSS_GENERATOR_COUNT:   usize = 3;
/// HP per generator (buffed hits, or one boss attack, damage it).
pub const BOSS_GENERATOR_HP:      i32   = 2;
/// Radius (px) of a generator node.
pub const BOSS_GENERATOR_R:       f32   = 95.0;
/// Generator colour (cyan — placeholder).
pub const C_BOSS_GENERATOR:       (u8, u8, u8) = (90, 220, 255);
/// Barrier colour (soft blue — placeholder).
pub const C_BOSS_BARRIER:         (u8, u8, u8) = (120, 160, 255);
/// Y (most-negative) the player/boss can't cross while the barrier is up.
pub const BOSS_BARRIER_Y:         f32   = -3600.0;
/// Y (more negative) the boss must cross after the barrier drops to fall into
/// the sun (the bait-and-bail finisher).
/// Ceiling the boss is clamped to during a lunge.
///
/// Was `BOSS_SUN_KILL_Y`: crossing it ENDED the fight, because the original
/// design let the player bait the boss into the sun. The sun is no longer part
/// of this battle — it is generators first, then buffed weakpoint damage — so
/// the line is now just the top of the arena and the lunge is clamped to it.
pub const BOSS_ARENA_TOP_Y:       f32   = -4300.0;
/// Ticks of telegraph before the boss's final desperation lunge.
pub const BOSS_LUNGE_TELEGRAPH:   u32   = 90;

// ── Gravity cannon hyper-transit (fast travel) ────────────────────────────────
/// Coin cost to use a cannon as fast travel.
pub const CANNON_FAST_TRAVEL_COST:      u32   = 300;
/// How far ahead a cannon fast-travel launches the player (px).
pub const CANNON_FAST_TRAVEL_DISTANCE:  f32   = VW * 3.0;
/// Ticks of no-grab grace after arriving at the receiver.
pub const CANNON_FAST_TRAVEL_GRACE:     u32   = 45;
/// Outgoing warp speed-line phase length (before the teleport).
pub const CANNON_WARP_OUT_TICKS:        i32   = 22;
/// Incoming (reverse) warp speed-line phase length (after the teleport).
pub const CANNON_WARP_IN_TICKS:         i32   = 28;
// Movement pattern speeds for lissajous figure-8
pub const BOSS_PHASE_X_SPEED:    f32   = 0.024;    // radians per tick (horizontal sweep)
pub const BOSS_PHASE_Y_SPEED:    f32   = 0.048;    // radians per tick (vertical — 2× for figure-8)
pub const BOSS_ARENA_HALF_W:     f32   = (BOSS_ZONE_X2 - BOSS_ZONE_X1) * 0.52; // amplitude — uses 52% for full traversal
pub const BOSS_ARENA_CENTER_X:   f32   = (BOSS_ZONE_X1 + BOSS_ZONE_X2) * 0.5;
pub const BOSS_Y_CENTER:         f32   = -2500.0;  // HUD Y ≈ -2500 (upper sky)
pub const BOSS_Y_AMPLITUDE:      f32   = 700.0;    // boss sweeps a wider vertical band
pub const C_BOSS_BODY:           (u8,u8,u8) = (60, 20, 200);   // deep purple
pub const C_BOSS_BOLT:           (u8,u8,u8) = (255, 110, 20);  // hot orange
pub const C_BOSS_HP_FILL:        (u8,u8,u8) = (220, 40,  40);  // red fill
pub const C_BOSS_HP_BG:          (u8,u8,u8) = (40,  10,  10);  // dark bg
/// Ominous name for the first boss (fits the sun/devourer theme).
pub const BOSS_NAME: &str = "THE SUN DEVOURER";
/// Lower bound on camera zoom while the boss fight is active. The Dune-style
/// height zoom would otherwise pull the camera way out on the tall arena; this
/// keeps the boss readable (less zoomed out).
pub const BOSS_CAM_MIN_ZOOM:      f32   = 0.8;
/// Diameter of the portal wormhole shown as the player is warped into the boss
/// arena (matches the visible wormhole portal used elsewhere).
pub const BOSS_WORMHOLE_D:        f32   = 1000.0;
/// Diameter of the huge black-hole threshold marker at the boss teleport, so the
/// player has a clear visual that they are heading into something special.
pub const BOSS_MARKER_D:          f32   = 3200.0;
/// Y-centre of the boss teleport threshold marker (covers the approach path).
pub const BOSS_MARKER_Y:          f32   = 1000.0;
// ── Solar eclipse (boss approach) ────────────────────────────────────────────
/// How far before a boss teleporter the eclipse begins. At ~300 px/s of real
/// play this is a little under three minutes of build-up.
pub const BOSS_ECLIPSE_RANGE: f32 = 50_000.0;
/// How far before the teleporter the eclipse has fully lifted.
///
/// The dark peaks here and releases over the last stretch, so the player
/// arrives at the black hole in daylight. Running the darkness right into the
/// teleport made the two events read as one; separating them lets the eclipse
/// be its own beat that passes, and leaves the black hole clearly visible at
/// the moment it matters.
pub const BOSS_ECLIPSE_RELEASE: f32 = 5_000.0;
/// Fraction of the darkening ramp spent at full light with the warning up,
/// before the dark starts falling. The warning still has to land before the
/// world reacts, but the wait was too long in play — the approach felt like it
/// was doing nothing for most of its length.
pub const ECLIPSE_WARN_FRACTION: f32 = 0.10;
/// Ambient strength at the darkest point.
///
/// 0.14 still lit the whole level enough to play by, so the player's lamp was
/// decoration rather than the thing you see by. Low enough now that outside the
/// lamp there is effectively nothing — but not zero, because the danger floor
/// has to stay findable.
pub const ECLIPSE_MIN_AMBIENT: f32 = 0.03;
/// Fraction of the darkening ramp by which full darkness is reached. Past this
/// the world holds at `ECLIPSE_MIN_AMBIENT` instead of continuing to creep
/// down, so most of the eclipse is spent AT its look rather than approaching it.
pub const ECLIPSE_FULL_DARK_AT: f32 = 0.5;
/// The player's lamp during the eclipse.
///
/// SIZED FROM THE TRAIL, because the trail is what makes a wrong size obvious.
/// The mid trail emitter lives 0.40 s, so at a cruising ~45 px/tick it streams
/// roughly 1 080 px behind the player. A lamp smaller than that lights the near
/// half of the trail and leaves the rest in the dark, which reads as a bug
/// rather than as lighting — the first pass at 5.5 player-widths (638 px) did
/// exactly that. The pass before it went the other way at a flat 2 600 px,
/// wider than the viewport, so nothing was dark at all.
pub const PLAYER_TRAIL_LIFETIME_S: f32 = 0.40;
/// Cruising speed the lamp is sized against (px/tick).
pub const ECLIPSE_LAMP_REF_SPEED: f32 = 45.0;
/// Slack past the trail's tail so its end fades inside the light, not at its edge.
pub const ECLIPSE_LAMP_MARGIN: f32 = 220.0;
pub const ECLIPSE_PLAYER_LIGHT_R: f32 =
    PLAYER_TRAIL_LIFETIME_S * 60.0 * ECLIPSE_LAMP_REF_SPEED + ECLIPSE_LAMP_MARGIN;

/// A second, wider, much dimmer light on the player.
///
/// A single point light is brightest at its centre and falls away fast, so the
/// player sat in a hotspot with the trail dimming out behind them. The fill
/// raises the base level across the whole lamp so the falloff reads as gentle
/// instead of as a spotlight. It deliberately does NOT cast shadows — one
/// shadow-casting source is what keeps the shadows DEFINED rather than doubled
/// and soft.
pub const ECLIPSE_FILL_LIGHT_R: f32 = ECLIPSE_PLAYER_LIGHT_R * 2.0;
pub const ECLIPSE_FILL_LIGHT_INTENSITY: f32 = 0.55;
pub const ECLIPSE_PLAYER_LIGHT_INTENSITY: f32 = 2.4;
/// Faint marker lights on the nearest grab nodes, so the route stays readable
/// without revealing the hazards.
pub const ECLIPSE_NODE_LIGHT_R: f32 = 460.0;
pub const ECLIPSE_NODE_LIGHT_INTENSITY: f32 = 0.55;
/// How often the eclipse re-ranks nearby nodes and re-flags shadow casters.
/// Every 6 frames (10 Hz) — nodes drift slowly relative to the player, so the
/// staleness is invisible while the saving is most of the effect's cost.
pub const ECLIPSE_LIGHT_REFRESH_TICKS: u32 = 6;

/// How many objects may be flagged as shadow occluders at once.
///
/// THE RENDERER CAPS OCCLUDERS AT 32 (`wgpu_canvas::gpu_types::MAX_OCCLUDERS`)
/// and silently drops the rest, and quartz collects them in object-store order
/// rather than by distance — so which 32 survive is arbitrary. Flagging every
/// pad, spinner, turret and asteroid in range blew past that, which is why
/// spinners cast shadows and pads did not: the pads were simply past the cut.
///
/// 20 leaves headroom for anything else in the scene that sets `shadow_caster`,
/// and shadows are only visible inside the lamp anyway, so the nearest 20 are
/// the only ones that could have been seen.
pub const ECLIPSE_MAX_SHADOW_CASTERS: usize = 20;

/// Gravity wells light themselves rather than casting shadows: they are a
/// hazard the player must see coming even when the lamp is nowhere near them,
/// and a well-shaped hole in the light reads as geometry rather than danger.
pub const ECLIPSE_GWELL_LIGHT_COUNT: usize = 6;
pub const ECLIPSE_GWELL_LIGHT_R: f32 = 620.0;
pub const ECLIPSE_GWELL_LIGHT_INTENSITY: f32 = 0.85;

/// Size of the down-pointing arrow above the black-hole threshold marker.
pub const BOSS_MARKER_ARROW_D:    f32   = 220.0;
/// How far before the boss threshold dedicated approach grapple nodes are
/// placed, so the player always has a swing path up to the portal.
/// How far before the teleport threshold the black-hole marker and its arrow
/// appear. At ~300 px/s of real play 20 000 px is about a minute of approach —
/// long enough to see it coming and commit, where the old 6 000 px was a few
/// seconds and easy to miss entirely.
pub const BOSS_APPROACH_RANGE:    f32   = 20_000.0;

// ── Comets ────────────────────────────────────────────────────────────────────
pub const COMET_POOL_SIZE:        usize = 8;
pub const COMET_SIZE:             f32   = 840.0;
pub const COMET_SPEED:            f32   = 84.0;
pub const COMET_LIFETIME:         u32   = 360;    // 6 s at 60 fps
/// Collision radius — smaller than the sprite so only the core fire cone hits.
pub const COMET_HIT_RADIUS:       f32   = 180.0;
/// Min vertical offset above player when spawning (world units).
pub const COMET_SPAWN_ABOVE:      f32   = 1000.0;
/// Max additional above offset so comets can come from varying heights.
pub const COMET_SPAWN_ABOVE_EXTRA: f32  = 800.0;
/// Horizontal spread from player centre on spawn.
pub const COMET_SPAWN_SPREAD:     f32   = 1600.0;
/// Knockback impulse applied to player on hit.
pub const COMET_KNOCKBACK:        f32   = 30.0;
pub const COMET_FPS:              f32   = 16.0;

// ── Comet warning indicator ───────────────────────────────────────────────────
pub const COMET_WARN_POOL_SIZE:   usize = 8;   // must equal COMET_POOL_SIZE
pub const COMET_WARN_W:           f32   = 200.0;
pub const COMET_WARN_H:           f32   = 400.0;
/// Total warning duration in ticks (2 s).
pub const COMET_WARN_TOTAL:       u32   = 120;
/// Tick at which phase 1 ends and phase 2 begins (1 s).
pub const COMET_WARN_P1_END:      u32   = 60;
/// Phase 1: ticks per image alternation (fast flash).
pub const COMET_WARN_ALT:         u32   = 4;
/// Phase 2 sub-boundaries (within phase 2, offset from P1_END):
/// 0..20 = light_explode, 20..40 = dark_explode, 40..60 = light_explode.
pub const COMET_WARN_P2_A:        u32   = 20;
pub const COMET_WARN_P2_B:        u32   = 40;

pub const BULLET_W:             f32 = 36.0;
pub const BULLET_H:             f32 = 12.0;
pub const BULLET_SPEED:         f32 = 52.0;  // phase 1 enhancement: significantly faster bullets
pub const BULLET_LIFETIME_TICKS:u32 = 300; // 5 seconds at 60fps
pub const C_TURRET_BODY:        (u8,u8,u8) = (100, 100, 130);
pub const C_TURRET_BARREL:      (u8,u8,u8) = (70, 70, 90);
pub const C_TURRET_BULLET:      (u8,u8,u8) = (220, 40, 40);

// ── Passive score dead-block ───────────────────────────────────────────
/// Width of one score-block (px).
pub const PASSIVE_SCORE_BLOCK_SIZE:  f32 = 5000.0;
/// Ticks of continuous presence (unpaused) before a block is marked dead.
/// 720 ticks = 12 seconds at 60 fps.
pub const PASSIVE_SCORE_DEAD_TICKS:  u32 = 720;

// ── Hearts / Checkpoint respawn ─────────────────────────────────────────────
/// Hearts a run starts with. Each fall costs one; zero ends the run.
pub const MAX_HEARTS: i32 = 3;
/// Distance (px) between auto-progress checkpoints. Each completed block saves
/// the nearest grab node as the respawn point.
pub const CHECKPOINT_INTERVAL: f32 = 5000.0;
/// Orbital radius for the respawn "come back in" animation.
pub const RESPAWN_ORBIT_R: f32 = 240.0;
/// Heart HUD geometry.
pub const HEART_W: f32 = 96.0;
pub const HEART_H: f32 = 88.0;
pub const HEART_GAP: f32 = 30.0;
// Placed to the right of the coin counter (which occupies x 26..666) so the two
// HUDs don't compete for the same top-left corner.
pub const HEART_HUD_X: f32 = 700.0;
pub const HEART_HUD_Y: f32 = 40.0;
pub const C_HEART_FULL:  (u8, u8, u8) = (240,  70,  80);
pub const C_HEART_EMPTY: (u8, u8, u8) = ( 60,  34,  38);

// ── Buff tether nodes ────────────────────────────────────────────────────────
/// Probability that a freshly-spawned grab node is a buff tether node.
pub const BUFF_HOOK_SPAWN_CHANCE: f32 = 0.05;
/// Minimum X distance between consecutive buff nodes (keeps them sparse).
pub const BUFF_HOOK_MIN_X_GAP: f32 = 6000.0;
/// Tag on buff tether nodes.
pub const BUFF_HOOK_TAG: &str = "buff_node";
/// How long a buff lasts (ticks). 600 = 10 s at 60 fps.
/// How long a tether buff lasts (5 s at 60 fps).
///
/// Halved from 600 after play: ten seconds of both weakpoint damage AND three
/// absorbed hits meant one buff node carried most of a fight, so the fight was
/// about reaching a node rather than about what you did once you had one.
pub const BUFF_DURATION_TICKS: u32 = 300;
/// How many boss projectiles a buff can absorb before it ends early.
pub const BUFF_ABSORB_MAX: u32 = 3;
/// Momentum cap granted while a buff is active (above the normal 56).
pub const BUFF_MOMENTUM_CAP: f32 = 84.0;
/// Buff node base colour (cyan — placeholder).
pub const C_BUFF_HOOK: (u8, u8, u8) = (110, 230, 255);

// ── Solar flare hazard + shielded nodes ──────────────────────────────────────
//
// A flare is a telegraphed, timed window during which the player must be
// TETHERED to a shielded node. Proximity is not enough — committing the tether
// is the counter-play, which makes the answer a swing decision rather than a
// position.
//
// The old implementation was inert in three ways, all fixed in `solar.rs`:
// the shelter test read every live hook rather than only tagged ones (so the
// tag conferred nothing), the `flare_warning` / `flare_active` canvas vars were
// written and never read by anything (so there was no telegraph and no visible
// flare — hearts just vanished every 40 s), and the heart cost was a single
// check on the eruption frame rather than a cost over a window.

/// Tag on shielded nodes.
pub const SHIELD_HOOK_TAG: &str = "shield_node";
/// Shielded node colour (gold).
pub const C_SHIELD_HOOK: (u8, u8, u8) = (255, 215, 90);

/// Maximum world-X gap between consecutive shielded nodes.
///
/// Expressed in DISTANCE, not in node count, because the guarantee the flare
/// needs is a distance one: `FLARE_SHELTER_SEARCH_AHEAD` must always find
/// something. A node-count cadence looked equivalent and was not — at ~520 px
/// per node, "every 20th node" put the first shelter 13 000 px into the run and
/// the flare correctly refused to fire for the entire opening.
///
/// 4 800 px is ~6 s of travel and roughly every 9th node: comfortably inside
/// the search window, frequent enough to learn from, rare enough that routing
/// to one is a decision rather than a formality.
pub const SHIELD_NODE_X_GAP: f32 = 4_800.0;

/// Ticks of telegraph before a flare erupts (3 s at 60 fps).
///
/// This never shrinks with difficulty. Shortening a reaction window reads as
/// unfair; shortening the gap between events reads as pressure, so the curve
/// scales `FLARE_INTERVAL_*` instead.
pub const FLARE_WARN_TICKS: u32 = 180;

/// Ticks the flare stays active (5 s).
pub const FLARE_ACTIVE_TICKS: u32 = 300;

/// Ticks between damage applications while unsheltered inside a flare (2 s).
pub const FLARE_DAMAGE_INTERVAL: u32 = 120;

/// Grace before the FIRST damage tick of a flare (1 s), so a player who reads
/// the telegraph late but reacts correctly is not punished for the read.
pub const FLARE_DAMAGE_GRACE: u32 = 60;

/// Ticks between flares at the easy and hard ends of the difficulty curve.
pub const FLARE_INTERVAL_EASY: f32 = 5400.0; // 90 s
pub const FLARE_INTERVAL_HARD: f32 = 2100.0; // 35 s

/// Radius of a shielded node's protective dome, used for the visual only —
/// shelter itself requires being tethered, not merely inside the ring.
pub const FLARE_SHIELD_RADIUS: f32 = 520.0;

/// How far ahead of the player a live shielded node must exist before a flare
/// is allowed to begin its telegraph. ~7 s of travel, comfortably more than the
/// 3 s warning, so a flare can never fire into a stretch with no shelter.
pub const FLARE_SHELTER_SEARCH_AHEAD: f32 = 6000.0;
/// How far behind the player a shelter still counts (backtracking is allowed).
pub const FLARE_SHELTER_SEARCH_BEHIND: f32 = 2200.0;

/// Ticks a flare's cooldown is extended by when no shelter is in range. Short,
/// so the flare fires as soon as the world offers an answer.
pub const FLARE_NO_SHELTER_RETRY: u32 = 90;

/// Screen tints for the two flare phases (RGBA, straight alpha at full phase).
pub const C_FLARE_WARN:   (u8, u8, u8, u8) = (255, 170,  60, 90);
pub const C_FLARE_ACTIVE: (u8, u8, u8, u8) = (255, 226, 150, 150);

/// Mega-shader bit for the player's protective dome while sheltered.
/// Matches `BIT_ENERGY_DOME` in `wgpu_canvas`'s `animated_vfx.wgsl`.
pub const MEGA_BIT_ENERGY_DOME: u32 = 1 << 20;

// ── Starfield background ──────────────────────────────────────────────────────
pub const STARFIELD_STAR_COUNT: u32 = 650;

// ── Rocket pad (rare special pad that launches player into space) ─────────────
pub const ROCKET_PAD_GAP_MIN:      f32   = 12000.0; // very wide gap → rare
pub const ROCKET_PAD_GAP_MAX:      f32   = 28000.0;
pub const ROCKET_PAD_POOL_SIZE:    usize = 8;
/// Probability that a normal pad spawn slot produces a rocket pad instead.
pub const ROCKET_PAD_SPAWN_CHANCE: f32   = 0.028;
pub const ROCKET_PAD_W:            f32   = 600.0;
pub const ROCKET_PAD_H:            f32   = 125.0;
/// Velocity applied to the player on rocket pad contact.
/// Must be large enough to clear the normal game zone entirely and reach
/// SPACE_ENTRY_Y. No natural swing + zero-g can match this force.
pub const ROCKET_PAD_LAUNCH_VY:    f32   = -165.0;
pub const ROCKET_PAD_LAUNCH_VX:    f32   = 22.0;
pub const C_ROCKET_PAD:            (u8,u8,u8) = (60, 220, 255);
pub const C_ROCKET_PAD_GLOW:       (u8,u8,u8) = (120, 240, 255);

// ── Space zone ────────────────────────────────────────────────────────────────
/// Player py must drop below this (negative y) to enter space mode.
pub const SPACE_ENTRY_Y:           f32 = -(VH * 2.40);
/// Depth at which the entry catch planet is centered and momentum is zeroed.
/// Must be below (more negative than) SPACE_ENTRY_Y by enough that the player
/// reaches it while still moving upward. Planet radius + gravity_influence_mult
/// together ensure gravity pulls from here all the way back to SPACE_ENTRY_Y.
pub const SPACE_SETTLE_Y:          f32 = -(VH * 3.15);
/// Player py rising back above this (less negative) while in space triggers return.
/// Pushed well up (into deep -y) so the space zone ends far above the normal
/// zone and normal-zone content can never bleed into the space view.
pub const SPACE_RETURN_Y:          f32 = -(VH * 1.10);
/// If player drifts this far left of the space entry anchor, rescue-teleport.
pub const SPACE_LEFT_BOUNDARY_MARGIN: f32 = VW * 0.95;
/// Target X range (relative to entry anchor) for left-boundary rescue teleport.
pub const SPACE_LEFT_TELEPORT_X_MIN: f32 = VW * 0.45;
pub const SPACE_LEFT_TELEPORT_X_MAX: f32 = VW * 1.05;
/// Right edge of the explorable space zone. A wormhole wraps the player back
/// before this so the special space zone stays bounded (and never drifts into
/// boss territory). Generous to keep space feeling enormous.
pub const SPACE_RIGHT_BOUNDARY_MARGIN: f32 = VW * 6.0;
pub const SPACE_RIGHT_TELEPORT_X_MIN: f32 = VW * 0.45;
pub const SPACE_RIGHT_TELEPORT_X_MAX: f32 = VW * 1.05;
/// Global gravity scale while in space — effectively zero. Planet and
/// black hole gravity wells supply all meaningful attraction in space.
pub const SPACE_GRAVITY_SCALE:     f32 = 0.002;
/// Oxygen timer in ticks (70 seconds at 60 fps).
pub const SPACE_OXYGEN_TICKS:      u32 = 4200;
/// Return boost applied when oxygen hits zero (strong downward push).
pub const SPACE_RETURN_FORCE_VY:   f32 = 55.0;
/// Welcome text display duration in ticks.
pub const SPACE_WELCOME_TICKS:     u32 = 200;
/// Ticks after oxygen depletion before forced return (grace period for "hold on").
pub const SPACE_RETURN_DELAY_TICKS: u32 = 90;

// Space object pool sizes
pub const SPACE_PLANET_POOL_SIZE:    usize = 24;
pub const SPACE_HOOK_POOL_SIZE:      usize = 160;
pub const SPACE_COIN_POOL_SIZE:      usize = 80;
pub const SPACE_BLACKHOLE_POOL_SIZE: usize = 8;
pub const SPACE_ASTEROID_POOL_SIZE:  usize = 80;

// Space object spawn budgets per tick
pub const SPACE_PLANET_SPAWN_BUDGET:    usize = 2;
pub const SPACE_HOOK_SPAWN_BUDGET:      usize = 8;  // one per Y-band per spawn tick
pub const SPACE_COIN_SPAWN_BUDGET:      usize = 0;
pub const SPACE_BLACKHOLE_SPAWN_BUDGET: usize = 1;
pub const SPACE_ASTEROID_SPAWN_BUDGET:  usize = 3;

// Space planet parameters
pub const SPACE_PLANET_GAP_MIN:         f32 = 1400.0;
pub const SPACE_PLANET_GAP_MAX:         f32 = 3200.0;
pub const SPACE_PLANET_Y_MIN:           f32 = -(VH * 5.0);
pub const SPACE_PLANET_Y_MAX:           f32 = -(VH * 1.7);
pub const SPACE_PLANET_RADIUS_SM_MIN:   f32 = 120.0;
pub const SPACE_PLANET_RADIUS_SM_MAX:   f32 = 220.0;
pub const SPACE_PLANET_RADIUS_LG_MIN:   f32 = 280.0;
pub const SPACE_PLANET_RADIUS_LG_MAX:   f32 = 460.0;
/// Gravity field extends this many times the visual radius.
pub const SPACE_PLANET_GRAV_R_MULT:     f32 = 1.3;
pub const SPACE_PLANET_GRAV_STRENGTH:   f32 = 0.5;

// Space hook parameters
pub const SPACE_HOOK_GAP_MIN:  f32 = 420.0;   // denser coverage
pub const SPACE_HOOK_GAP_MAX:  f32 = 920.0;
// Three vertical bands — shallow (entry), mid, and deep space.
// Each hook spawn tick picks one band randomly, ensuring recovery
// points are available even if the player flies deep into space.
pub const SPACE_HOOK_Y_SHALLOW_MIN: f32 = -(VH * 4.2);
pub const SPACE_HOOK_Y_SHALLOW_MAX: f32 = -(VH * 1.8);
pub const SPACE_HOOK_Y_MID_MIN:     f32 = -(VH * 6.5);
pub const SPACE_HOOK_Y_MID_MAX:     f32 = -(VH * 4.0);
pub const SPACE_HOOK_Y_DEEP_MIN:    f32 = -(VH * 10.0);
pub const SPACE_HOOK_Y_DEEP_MAX:    f32 = -(VH * 6.0);
// Keep old names as aliases so nothing else breaks
pub const SPACE_HOOK_Y_MIN: f32 = SPACE_HOOK_Y_SHALLOW_MIN;
pub const SPACE_HOOK_Y_MAX: f32 = SPACE_HOOK_Y_SHALLOW_MAX;
// Dense hook zone near the solar ceiling (0.5–2.0 screen-heights below the sun).
pub const SPACE_HOOK_SUN_SAFE_MIN_FROM_KILL: f32 = ROPE_LEN_MAX * 2.0;
pub const SPACE_HOOK_SUN_ZONE_Y_MIN: f32 = SPACE_UPPER_LIMIT_Y + SPACE_HOOK_SUN_SAFE_MIN_FROM_KILL;
pub const SPACE_HOOK_SUN_ZONE_Y_MAX: f32 = SPACE_UPPER_LIMIT_Y + VH * 2.6;
pub const SPACE_HOOK_SUN_SAFETY_BAND_MIN: f32 = SPACE_UPPER_LIMIT_Y + ROPE_LEN_MAX * 2.1;
pub const SPACE_HOOK_SUN_SAFETY_BAND_MAX: f32 = SPACE_UPPER_LIMIT_Y + ROPE_LEN_MAX * 2.9;
pub const SPACE_HOOK_SUN_GAP_MIN:    f32 = 140.0;
pub const SPACE_HOOK_SUN_GAP_MAX:    f32 = 260.0;

// Space coin parameters
pub const SPACE_COIN_GAP_MIN:  f32 = 1400.0;
pub const SPACE_COIN_GAP_MAX:  f32 = 2600.0;
pub const SPACE_COIN_SCORE:    u32 = 5;
pub const SPACE_CATCOIN_SCORE:      u32 = 5;
pub const SPACE_CATCOIN_BLUE_SCORE: u32 = 10;
pub const SPACE_CATCOIN_RED_SCORE:  u32 = 25;
pub const SPACE_CATCOIN_BLUE_CHANCE: f32 = 0.22;
pub const SPACE_CATCOIN_RED_CHANCE:  f32 = 0.08;
pub const SPACE_COIN_ANIM_FPS: f32 = 6.0;
pub const SPACE_COIN_R:        f32 = 27.0;
pub const SPACE_COIN_FORMATION_COUNT: usize = 4;
pub const SPACE_COIN_FORMATION_SPACING: f32 = 210.0;
pub const SPACE_COIN_FORMATION_ARC_RISE: f32 = 62.0;
pub const SPACE_COIN_FORMATION_Y_MIN: f32 = -(VH * 4.6);
pub const SPACE_COIN_FORMATION_Y_MAX: f32 = -(VH * 1.9);
pub const SPACE_PLANET_HOOK_GUIDE_COINS: usize = 4;
pub const SPACE_PLANET_HOOK_GUIDE_RED_CHANCE: f32 = 0.20;
pub const SPACE_PLANET_HOOK_GUIDE_T_MIN: f32 = 0.20;
pub const SPACE_PLANET_HOOK_GUIDE_T_MAX: f32 = 0.75;
pub const SPACE_PLANET_LINK_COINS: usize = 8;
pub const SPACE_PLANET_LINK_RED_CHANCE: f32 = 0.16;
pub const SPACE_PLANET_LINK_T_MIN: f32 = 0.18;
pub const SPACE_PLANET_LINK_T_MAX: f32 = 0.82;
pub const SPACE_SUN_BONUS_CLUSTER_CHANCE: f32 = 0.022;
pub const SPACE_SUN_BONUS_CLUSTER_COINS_MIN: usize = 6;
pub const SPACE_SUN_BONUS_CLUSTER_COINS_MAX: usize = 10;
pub const SPACE_SUN_BONUS_CLUSTER_SPACING: f32 = 96.0;
pub const SPACE_SUN_BONUS_CLUSTER_RING_R: f32 = 170.0;
pub const SPACE_SUN_BONUS_RED_CHANCE: f32 = 0.18;

// Space oxygen pickups (extend the oxygen meter)
pub const SPACE_OXYGEN_PICKUP_POOL_SIZE: usize = 24;
pub const SPACE_OXYGEN_PICKUP_GAP_MIN: f32 = 2600.0;
pub const SPACE_OXYGEN_PICKUP_GAP_MAX: f32 = 5200.0;
pub const SPACE_OXYGEN_PICKUP_R: f32 = 88.0;
/// Ticks of oxygen a canister restores (10 s at 60 fps).
pub const SPACE_OXYGEN_PICKUP_AMOUNT: u32 = 600;
pub const SPACE_OXYGEN_PICKUP_SPAWN_BUDGET: usize = 1;
pub const SPACE_OXYGEN_PICKUP_Y_MIN: f32 = -(VH * 3.6);
pub const SPACE_OXYGEN_PICKUP_Y_MAX: f32 = -(VH * 0.9);

// ── Roguelike upgrade nodes (spend coins for run boosts) ─────────────────────
pub const UPGRADE_POOL_SIZE: usize = 12;
/// Longest the post-dialogue stasis may hold the player waiting for a tether
/// before releasing them anyway (8 s). The hold is meant to end on a grab, and
/// `close_dialogue` guarantees a node in reach — this only fires if that
/// guarantee fails, and turns a permanent soft-lock into a survivable fall.
pub const UPGRADE_HOLD_MAX_TICKS: u32 = 480;
pub const UPGRADE_GAP_MIN: f32 = 30000.0;
pub const UPGRADE_GAP_MAX: f32 = 55000.0;
pub const UPGRADE_R: f32 = 96.0;
pub const UPGRADE_SPAWN_BUDGET_PER_TICK: usize = 1;
/// Run-persisting upgrades: cheap first buy, escalating per purchase this run.
pub const UPGRADE_RUN_HEART_BASE: u32 = 200;
pub const UPGRADE_RUN_HEART_GROWTH: f32 = 2.0;
pub const UPGRADE_BREATH_BASE: u32 = 150;
pub const UPGRADE_BREATH_GROWTH: f32 = 1.6;
pub const UPGRADE_MOMENTUM_BASE: u32 = 200;
pub const UPGRADE_MOMENTUM_GROWTH: f32 = 1.6;
/// Permanent extra-heart upgrade (persists across runs; meta currency, exponential).
pub const UPGRADE_PERM_HEART_BASE: u64 = 5000;
pub const UPGRADE_PERM_HEART_GROWTH: f32 = 2.5;
/// "Controlled breathing" — oxygen drains at this fraction of normal.
pub const UPGRADE_BREATH_DRAIN_SCALE: f32 = 0.72;
/// Momentum cap while the momentum upgrade is owned.
pub const UPGRADE_MOMENTUM_CAP: f32 = 70.0;
pub const C_UPGRADE: (u8, u8, u8) = (200, 120, 255);
/// Game var holding how many permanent extra hearts are owned.
pub const META_EXTRA_HEARTS_VAR: &str = "meta_extra_hearts";
/// Meta currency awarded (and shown) when the boss is defeated, for permanent
/// roguelike upgrades.
pub const META_BOSS_REWARD: u64 = 50;

// Black hole parameters
pub const SPACE_BLACKHOLE_GAP_MIN:       f32 = 5000.0;
pub const SPACE_BLACKHOLE_GAP_MAX:       f32 = 9000.0;
pub const SPACE_BLACKHOLE_RADIUS_MIN:    f32 = 100.0;
pub const SPACE_BLACKHOLE_RADIUS_MAX:    f32 = 200.0;
pub const SPACE_BLACKHOLE_GRAV_STRENGTH: f32 = 0.7;
pub const SPACE_BLACKHOLE_VISUAL_RADIUS_MULT: f32 = 3.0;
pub const SPACE_BLACKHOLE_INFLUENCE_RADIUS_MULT: f32 = 2.2;
pub const SPACE_BLACKHOLE_TELEPORT_CORE_FRAC: f32 = 0.34;
pub const SPACE_BLACKHOLE_TELEPORT_SAFE_FROM_SUN: f32 = 520.0;
pub const SPACE_BLACKHOLE_TELEPORT_SAFE_FROM_RETURN: f32 = 680.0;
pub const SPACE_BLACKHOLE_TELEPORT_X_OFFSET_MIN: f32 = VW * 0.18;
pub const SPACE_BLACKHOLE_TELEPORT_X_OFFSET_MAX: f32 = VW * 0.45;
pub const SPACE_BLACKHOLE_TELEPORT_Y_OFFSET_MIN: f32 = VH * 0.95;
pub const SPACE_BLACKHOLE_TELEPORT_Y_OFFSET_MAX: f32 = VH * 2.2;
pub const SPACE_BLACKHOLE_TELEPORT_BLUE_TICKS: u32 = 52;
pub const SPACE_BLACKHOLE_TELEPORT_DORMANT_TICKS: u32 = 62;
pub const SPACE_BLACKHOLE_Y_MIN:         f32 = -(VH * 2.8);
pub const SPACE_BLACKHOLE_Y_MAX:         f32 = -(VH * 0.55);

// Decorative asteroid parameters (main gameplay area)
pub const SPACE_ASTEROID_GAP_MIN:        f32 = 1300.0;
pub const SPACE_ASTEROID_GAP_MAX:        f32 = 2800.0;
// Small asteroids float near the hook zone; large ones drift higher.
// Y is interpolated between these two bands based on normalised size.
// Both bands sit well above SPACE_RETURN_Y so nothing crowds the normal zone.
pub const SPACE_ASTEROID_Y_NEAR_MIN:     f32 = -3400.0;  // small, mid-space
pub const SPACE_ASTEROID_Y_NEAR_MAX:     f32 = -2600.0;
pub const SPACE_ASTEROID_Y_FAR_MIN:      f32 = -4400.0; // large, highest (visible zoomed-out)
pub const SPACE_ASTEROID_Y_FAR_MAX:      f32 = -2800.0;
pub const SPACE_ASTEROID_SIZE_MIN:       f32 = 180.0;
pub const SPACE_ASTEROID_SIZE_MAX:       f32 = 480.0;
/// Base outward knockback speed applied to player when hit by a space asteroid.
pub const ASTEROID_PLAYER_KNOCKBACK_BASE: f32 = 26.0;
/// Extra knockback from relative closing speed along the collision normal.
pub const ASTEROID_PLAYER_KNOCKBACK_IMPACT: f32 = 1.10;
/// Clamp for total asteroid hit knockback to avoid absurd launch speeds.
pub const ASTEROID_PLAYER_KNOCKBACK_MAX: f32 = 130.0;
/// How much asteroid velocity is carried into player velocity on hit.
pub const ASTEROID_PLAYER_KNOCKBACK_CARRY: f32 = 0.60;
/// Crystalline collision layer bits.
pub const ASTEROID_COLLISION_LAYER: u32 = 1 << 8;
pub const PLAYER_COLLISION_LAYER:   u32 = 1 << 1; // matches collision_layers::PLAYER

// ── Spawn-build animation ─────────────────────────────────────────────────────
/// Duration of the drop-in animation (frames).
pub const SPAWN_ANIM_TICKS: u32 = 150;
/// How far above target the object starts (virtual pixels).
/// ~VH/3.5 — places the start near the top of the camera view so the
/// full drop is visible rather than happening off-screen above the player.
pub const SPAWN_ANIM_DROP:  f32 = 600.0;

// Camera behavior during space transition
pub const SPACE_CAM_LERP_IN:    f32 = 0.048;  // slower lerp (dramatic ascent)
pub const SPACE_CAM_ZOOM_IN:    f32 = 0.82;   // pull back in space for wider visibility/scale
pub const SPACE_CAM_Y_LEAD:     f32 = VH * 0.12; // lead camera above player

// Space color palette
pub const C_SPACE_PLANET: [(u8,u8,u8); 5] = [
    (215, 115, 55),  // Rust/Mars
    (75, 155, 235),  // Ice-blue
    (175, 75, 215),  // Purple gas giant
    (95, 215, 155),  // Green-teal
    (235, 210, 90),  // Sandy/yellow
];
pub const C_SPACE_COIN:  (u8,u8,u8) = (255, 230, 100);
pub const C_SPACE_COIN_HIGH: (u8,u8,u8) = (120, 255, 220);
pub const C_SPACE_HOOK:  (u8,u8,u8) = (155, 115, 255);
pub const C_SPACE_HOOK_ON: (u8,u8,u8) = (210, 185, 255);
pub const SPECIAL_HOOK_TAG: &str = "hook_special";
pub const SPECIAL_HOOK_SPAWN_CHANCE: f32 = 0.30;
pub const SPECIAL_HOOK_MIN_X_GAP: f32 = 10_000.0;
pub const C_HOOK_EXTENDED:      (u8,u8,u8) = (220, 60, 80);
pub const C_HOOK_EXTENDED_NEAR: (u8,u8,u8) = (255, 120, 140);
pub const C_HOOK_EXTENDED_ON:   (u8,u8,u8) = (255, 180, 200);
pub const EXTENDED_HOOK_TAG: &str = "hook_extended";
pub const EXTENDED_HOOK_REACH_MULT: f32 = 2.0;
pub const EXTENDED_HOOK_MIN_X_GAP: f32 = 20_000.0;
pub const EXTENDED_HOOK_SPAWN_CHANCE: f32 = 0.08;
pub const POWERUP_MAGNET_RADIUS: f32 = 160.0;
pub const POWERUP_MAGNET_PULL: f32 = 0.35;
pub const C_BLACKHOLE:   (u8,u8,u8) = (18,  8,   26);
pub const C_GWELL_TELEPORT: (u8,u8,u8) = (90, 170, 255);

// Oxygen HUD bar
pub const OXYGEN_BAR_W:  f32 = 700.0;
pub const OXYGEN_BAR_H:  f32 = 42.0;
pub const C_OXY_FULL:    (u8,u8,u8) = (80,  220, 160);
pub const C_OXY_MID:     (u8,u8,u8) = (240, 200, 55);
pub const C_OXY_LOW:     (u8,u8,u8) = (220, 55,  55);

// ── Space zone — new features ─────────────────────────────────────────────────

/// Momentum cap while in space mode (2/3 of the normal cap).
pub const SPACE_MOMENTUM_CAP: f32 = MOMENTUM_CAP * 0.5;

/// Y coordinate of the solar ceiling (5 screen-heights above space entry).
/// Solar gif is placed here; crossing into the dense surface zone triggers sun-death.
pub const SPACE_UPPER_LIMIT_Y: f32 = SPACE_ENTRY_Y - VH * 5.0;

/// Approximate height of corona_v5.gif when scaled to full VW width.
/// Adjust if the gif has a different aspect ratio.
pub const SPACE_SOLAR_H: f32 = VH * 1.0;

/// Distance from the killline where the solar ceiling reveal starts.
/// Set to cover the entire space zone so corona is visible from entry.
pub const SPACE_SOLAR_REVEAL_DIST: f32 = VH * 4.2;
/// Far-away scale: keep corona at native screen width (no zoom-in effect).
pub const SPACE_SOLAR_FAR_SCALE: f32 = 1.0;
/// Bottom Y of the corona in screen-space when the player is far from the sun.
/// VH*0.08 delays initial visibility so the sun does not appear too early.
pub const SPACE_SOLAR_FAR_BOTTOM_OFFSET: f32 = VH * 0.08;
/// Bottom Y of the solar ceiling when fully revealed (screen-space).
/// VH*0.90 brings the dense surface line into view right as killline is reached.
pub const SPACE_SOLAR_NEAR_BOTTOM_Y: f32 = VH * 0.90;

/// Default solar surface ratio (y from top / height), derived from a frame-wide
/// luminance scan of corona_v5.gif (lum>=120, row coverage>=0.35).
pub const SOLAR_SURFACE_RATIO_DEFAULT: f32 = 0.3690;

/// Animation speed for the solar ceiling gif (fps).
pub const SOLAR_ANIM_FPS: f32 = 8.0;

/// Asset path for the solar ceiling gif.
pub const ASSET_SOLAR_GIF: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/corona_v5.gif");

// Red (arc) coins
pub const SPACE_BLUE_COIN_POOL_SIZE: usize = 20;
pub const SPACE_RED_COIN_POOL_SIZE: usize = 20;
/// Score awarded for collecting a red space coin.
pub const SPACE_RED_COIN_SCORE:     u32   = 3000;
/// Visual radius of a red space coin (slightly larger than normal space coin).
pub const SPACE_RED_COIN_R:         f32   = 29.0;

// Planet coin arcs — when a space planet spawns it places coins in a ring.
/// Number of coins placed in the arc around each planet.
pub const SPACE_COIN_ARC_COUNT:        usize = 5;
/// Fraction of arc coins that are red (floored to whole coins).
pub const SPACE_COIN_ARC_RED_FRAC:     f32   = 0.20;
/// Distance from planet centre where the arc coins are placed (×visual_r).
pub const SPACE_COIN_ARC_RADIUS_MULT:  f32   = 1.85;
/// Number of hooks placed near each newly spawned space planet.
pub const SPACE_PLANET_NEARBY_HOOKS:   usize = 3;
/// Offset from planet centre to nearby hook positions (px beyond visual_r).
pub const SPACE_PLANET_HOOK_OFFSET:    f32   = 340.0;

// Space gravity wells (repurpose blackhole pool)
/// Number of hooks placed near each newly spawned space gravity well.
pub const SPACE_GWELL_NEARBY_HOOKS:   usize = 2;
/// Offset from well centre to nearby hook positions (px).
pub const SPACE_GWELL_HOOK_OFFSET:    f32   = 500.0;

// Space planet orbit capture (near-surface autopilot)
/// Distance from planet surface where orbit capture begins (px).
pub const SPACE_PLANET_ORBIT_CAPTURE_PAD: f32 = 120.0;
/// Locked orbit altitude from planet surface while captured (px).
pub const SPACE_PLANET_ORBIT_ALT_PAD: f32 = 140.0;
/// Minimum tangential speed retained for stable CW/CCW orbit (px/tick).
pub const SPACE_PLANET_ORBIT_MIN_TANGENTIAL: f32 = 8.0;
/// Maximum tangential speed allowed while orbiting (px/tick).
pub const SPACE_PLANET_ORBIT_MAX_TANGENTIAL: f32 = 42.0;
/// Tangential drag while orbiting (keeps long orbits stable).
pub const SPACE_PLANET_ORBIT_DRAG: f32 = 0.997;

// Asteroid drift — velocity components added when an asteroid is spawned.
pub const SPACE_ASTEROID_VX_MIN: f32 = -4.0;
pub const SPACE_ASTEROID_VX_MAX: f32 =  4.0;
pub const SPACE_ASTEROID_VY_MIN: f32 = -2.0;
pub const SPACE_ASTEROID_VY_MAX: f32 =  2.0;
// Fraction of the player's incoming velocity transferred to an asteroid on hook.
// Scaled by (SIZE_MIN / actual_size) so smaller asteroids receive more impulse.
pub const ASTEROID_HOOK_IMPULSE_FACTOR: f32 = 0.28;
/// Tag on asteroids that drift through the grab-node band (as opposed to the
/// ones parked high above it). Both are tetherable; only these are in the way.
pub const ASTEROID_DRIFT_TAG: &str = "asteroid_drift";

/// Scales the player's closing speed into an asteroid body-hit impulse.
/// impulse = base 1.5 + closing_speed * factor (capped at 22.0 px/frame).
pub const ASTEROID_PLAYER_BODY_IMPULSE_FACTOR: f32 = 0.30;

// Stasis orbit (shared between entry/exit stasis and game-start stasis)
pub const STASIS_ORBIT_R:     f32 = 240.0;
pub const STASIS_ORBIT_OMEGA: f32 = 0.038;

// ── Gravity Cannon obstacle ───────────────────────────────────────────────────
pub const GRAVITYCANNON_W:               f32   = 300.0;
pub const GRAVITYCANNON_H:               f32   = 300.0;
pub const GRAVITYCANNON_FPS:             f32   = 8.0;
pub const GRAVITYCANNON_FRAME_COUNT:     usize = 9;  // frames 0–8
pub const CANNON_DEFAULT_FRAME_INDEX:    usize = 8;  // frame 9 (1-based)
pub const CANNON_POOL_SIZE:              usize = 4;
pub const CANNON_GAP_MIN:                f32   = 8000.0;
pub const CANNON_GAP_MAX:                f32   = 14000.0;
pub const CANNON_DEFAULT_ROTATION:       f32   = -90.0;  // 90° CCW
pub const CANNON_BOB_AMP:                f32   = 35.0;   // px
pub const CANNON_BOB_SPEED:              f32   = 0.055;  // rad/tick ≈ 1.9 rad/s at 60 fps
pub const CANNON_TRIGGER_RADIUS:         f32   = 240.0;
pub const CANNON_PULL_RADIUS:            f32   = 520.0;
pub const CANNON_PULL_ACCEL:             f32   = 2.80;   // per-tick pull at strongest
pub const CANNON_PULL_SPEED_CAP:         f32   = 72.0;   // cap speed while being pulled
pub const CANNON_CAPTURE_TICKS_PER_FRAME: u32  = 5;     // pulse frames 8→7→6→7→8
/// How long the cannon holds the player for the fast-travel choice before the
/// default launch fires. 120 ticks ≈ 2 s at 60 fps.
pub const CANNON_CHOICE_WAIT_TICKS:      u32   = 120;
pub const CANNON_CHARGE_TICKS:           u32   = 40;    // hold player in barrel
pub const CANNON_CHARGE_ROTATION_DEG:    f32   = 50.0;  // CW rotation during charge
pub const CANNON_FIRE_TICKS_PER_FRAME:   u32   = 5;     // frames 8→0 (slower so the launch reads clearly)
pub const CANNON_LAUNCH_VX:              f32   = 124.0; // very long forward shot
pub const CANNON_LAUNCH_VY:              f32   = -38.0; // stronger upward arc
pub const CANNON_GRAVITY_DAMP_TICKS:     u32   = 180;   // longer reduced gravity after launch
pub const GRAVITY_DAMP_SCALE:            f32   = 0.03;  // gravity multiplier during damp
pub const CANNON_RECOVER_TICKS:          u32   = 60;    // rotate back to default rotation
pub const LAYER_CANNON_ACTIVE:           i32   = 60;    // above player layer (42)
pub const ASSET_GRAVITYCANNON_GIF: &[u8] = include_bytes!("../assets/gravitycannon.gif");
