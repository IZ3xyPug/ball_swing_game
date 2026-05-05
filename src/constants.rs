#![allow(dead_code)]
// ── Virtual resolution ────────────────────────────────────────────────────────
pub const VW: f32 = 3840.0;
pub const VH: f32 = 2160.0;

// ── Physics ───────────────────────────────────────────────────────────────────
pub const GRAVITY:        f32 = 0.9;
pub const SWING_TENSION:  f32 = 1.08;
pub const MOMENTUM_CAP:   f32 = 56.0;
pub const ROPE_LEN_MIN:   f32 = 200.0;
pub const ROPE_LEN_MAX:   f32 = 720.0;
pub const SWING_DRAG:     f32 = 0.999;
pub const GRAB_SURGE:     f32 = 4.2;
pub const GRAB_TANGENT_SURGE_SCALE: f32 = 0.12;
pub const GRAB_TANGENT_SURGE_MAX:   f32 = 4.0;
pub const GRAB_SURGE_MULT: f32 = 2.6;
pub const GRAB_VERTICAL_BOOST: f32 = 1.28;
pub const GRAB_SPIN_DISABLE_SPEED: f32 = 50.0;
pub const RELEASE_MIN_SWING_SPEED: f32 = 3.2;
pub const RELEASE_SURGE_SCALE: f32 = 0.42;
pub const RELEASE_SURGE_MAX: f32 = 14.0;
pub const RELEASE_VERTICAL_BOOST: f32 = 1.42;

// ── Object sizes ──────────────────────────────────────────────────────────────
pub const PLAYER_R:       f32 = 40.0;
pub const HOOK_R:         f32 = 38.0;
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
pub const GEN_AHEAD:      f32 = VW * 2.5;

/// Max hooks the generator places per game tick (frame).
/// Higher = faster queue fill but more CPU per frame.
pub const HOOKS_SPAWN_BUDGET_PER_TICK:    usize = 2;
pub const PADS_SPAWN_BUDGET_PER_TICK:     usize = 2;
pub const SPINNERS_SPAWN_BUDGET_PER_TICK: usize = 2;
pub const FLIPS_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const ZERO_G_SPAWN_BUDGET_PER_TICK:   usize = 1;
pub const GATES_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const COIN_BATCHES_BUDGET_PER_TICK:   usize = 1;

// ── Generation — Grab Points (Hooks) ─────────────────────────────────────────

/// How many hooks the pending queue is filled to per batch call.
/// Increase → longer lookahead, smoother streaming.
pub const MAX_HOOKS_LIVE: usize = 10;

/// Object pool size. Must be ≥ (MAX_HOOKS_LIVE + starter hooks).
/// Increasing this is safe; decreasing below ~20 will cause pool starvation.
pub const HOOK_POOL_SIZE: usize = 68;

/// Horizontal distance between consecutive grab points (px).
/// This is the single most impactful spacing constant.
/// Increase → harder (longer reach required). Decrease → easier.
pub const HOOK_FIXED_X_GAP: f32 = 1250.0;

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

/// Hard minimum Euclidean distance between consecutive hook nodes.
/// = ROPE_LEN_MAX / 2 (300 px). No two successive hooks will be closer than this.
/// Hooks closer than this are too clustered to be interesting to swing between.
pub const HOOK_MIN_REACH: f32 = ROPE_LEN_MAX * 0.5; // 300.0

/// Hard maximum Euclidean distance between consecutive hook nodes.
/// = ROPE_LEN_MAX (600 px). Every hook must be reachable from the previous one.
/// Hooks farther than this create unreachable gaps — forbidden.
pub const HOOK_MAX_REACH: f32 = ROPE_LEN_MAX; // 600.0

/// Horizontal stride range per hop (px).
/// Intentionally set large to create long gaps between hooks.
/// This can exceed HOOK_MAX_REACH and break strict reachability by design.
pub const HOOK_X_STRIDE_MIN: f32 = 1160.0;
pub const HOOK_X_STRIDE_MAX: f32 = 1340.0;

// ── Generation — Bounce Pads ──────────────────────────────────────────────────

pub const PAD_POOL_SIZE:  usize = 32;

/// X gap between consecutive bounce pads (px). Wide range for variety.
/// Increase both to make pads rarer. Decrease for more frequent pads.
pub const PAD_GAP_MIN:    f32 = 5000.0;
pub const PAD_GAP_MAX:    f32 = 9000.0;

// techbouncernew.gif is decoded into this fixed gameplay footprint.
// Art scaling changes should happen in the loader, not by changing pad geometry.
pub const PAD_W:          f32 = 775.0;
pub const PAD_H:          f32 = 262.5;
/// techbouncernew.gif visual occupancy ratio inside the 256px source frame.
/// Used to keep bounce collision width aligned to visible pad art.
pub const PAD_COLLISION_WIDTH_FACTOR: f32 = 170.0 / 256.0;

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
pub const PAD_BOUNCE_VY: f32 = -52.0;

/// How far a moving pad travels from its origin (px). 0 = static.
pub const PAD_MOVE_RANGE: f32 = 250.0;
/// Speed of pad oscillation (px/tick).
pub const PAD_MOVE_SPEED: f32 = 3.0;

pub fn pad_corner_radius() -> f32 {
    // Tuned to the current bounce-pad art profile (rounded_rectangle + 9-slice).
    // At PAD_H=262.5 this yields ~66px.
    (PAD_H * 0.254).clamp(1.0, PAD_H * 0.5 - 1.0)
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

/// Distance (px) at which the zone advances (Normal → Purple → Black → repeat).
/// Increase for longer zone stretches. Decrease to cycle zones faster.
pub const ZONE_DISTANCE_STEP:f32 = 20000.0;

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
/// Radius of the coin magnet pickup effect (px).
pub const COIN_MAGNET_RADIUS:f32 = 180.0;
pub const COIN_MAGNET_PULL:  f32 = 0.37;

// ── Generation — Flip Pickups ─────────────────────────────────────────────────

pub const FLIP_POOL_SIZE:    usize = 16;
/// X gap between gravity-flip pickups (px). Increase = rarer flips.
pub const FLIP_GAP_MIN:      f32 = 14000.0;
pub const FLIP_GAP_MAX:      f32 = 24000.0;
pub const FLIP_W:            f32 = 110.0;
pub const FLIP_H:            f32 = 110.0;
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
pub const C_HOOK:     (u8,u8,u8) = (200, 60,  20 );
pub const C_HOOK_ON:  (u8,u8,u8) = (255, 90,  70 );
pub const C_HOOK_NEAR:(u8,u8,u8) = (255, 120, 50 );
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

// ── Asset paths ──────────────────────────────────────────────────────────────
pub const ASSET_COIN_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/coin.gif");
pub const ASSET_SCORE_X2_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/2x.gif");
pub const ASSET_TECH_BOUNCE_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/techbouncernew.gif");
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
pub const ASSET_BACKGROUND: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/background.png");
pub const ASSET_BACKGROUND_2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/background_2.webp");
pub const ASSET_AURORA_EARTH_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/aurora_earth.gif");
pub const ASSET_ASTEROID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/asteroid.webp");
pub const ASSET_THRUSTER1_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/thruster1.gif");
pub const ASSET_CALICOBALL_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/calicoball.gif");
pub const ASSET_BLACKHOLE1_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/blackhole1.gif");
pub const ASSET_WORMHOLE2_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/wormhole2.gif");
pub const ASSET_GWELLON_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gwellon.gif");
pub const ASSET_GWELLOFF_GIF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gwelloff.gif");
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
pub const GWELL_STRENGTH_MIN:  f32 = 0.75;
pub const GWELL_STRENGTH_MAX:  f32 = 1.0;

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

// ── Starfield background ──────────────────────────────────────────────────────
pub const STARFIELD_STAR_COUNT: u32 = 350;

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
pub const ROCKET_PAD_LAUNCH_VY:    f32   = -130.0;
pub const ROCKET_PAD_LAUNCH_VX:    f32   = 22.0;
pub const C_ROCKET_PAD:            (u8,u8,u8) = (60, 220, 255);
pub const C_ROCKET_PAD_GLOW:       (u8,u8,u8) = (120, 240, 255);

// ── Space zone ────────────────────────────────────────────────────────────────
/// Player py must drop below this (negative y) to enter space mode.
pub const SPACE_ENTRY_Y:           f32 = -(VH * 1.35);
/// Depth at which the entry catch planet is centered and momentum is zeroed.
/// Must be below (more negative than) SPACE_ENTRY_Y by enough that the player
/// reaches it while still moving upward. Planet radius + gravity_influence_mult
/// together ensure gravity pulls from here all the way back to SPACE_ENTRY_Y.
pub const SPACE_SETTLE_Y:          f32 = -(VH * 2.1);  // ~-4536 at VH=2160
/// Player py rising back above this (less negative) while in space triggers return.
pub const SPACE_RETURN_Y:          f32 = -(VH * 0.05);
/// If player drifts this far left of the space entry anchor, rescue-teleport.
pub const SPACE_LEFT_BOUNDARY_MARGIN: f32 = VW * 0.95;
/// Target X range (relative to entry anchor) for left-boundary rescue teleport.
pub const SPACE_LEFT_TELEPORT_X_MIN: f32 = VW * 0.45;
pub const SPACE_LEFT_TELEPORT_X_MAX: f32 = VW * 1.05;
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
pub const SPACE_ASTEROID_POOL_SIZE:  usize = 40;

// Space object spawn budgets per tick
pub const SPACE_PLANET_SPAWN_BUDGET:    usize = 2;
pub const SPACE_HOOK_SPAWN_BUDGET:      usize = 8;  // one per Y-band per spawn tick
pub const SPACE_COIN_SPAWN_BUDGET:      usize = 0;
pub const SPACE_BLACKHOLE_SPAWN_BUDGET: usize = 1;
pub const SPACE_ASTEROID_SPAWN_BUDGET:  usize = 3;

// Space planet parameters
pub const SPACE_PLANET_GAP_MIN:         f32 = 1400.0;
pub const SPACE_PLANET_GAP_MAX:         f32 = 3200.0;
pub const SPACE_PLANET_Y_MIN:           f32 = -(VH * 4.0);
pub const SPACE_PLANET_Y_MAX:           f32 = -(VH * 0.55);
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
pub const SPACE_HOOK_Y_SHALLOW_MIN: f32 = -(VH * 3.2);
pub const SPACE_HOOK_Y_SHALLOW_MAX: f32 = -(VH * 0.42);
pub const SPACE_HOOK_Y_MID_MIN:     f32 = -(VH * 5.5);
pub const SPACE_HOOK_Y_MID_MAX:     f32 = -(VH * 3.0);
pub const SPACE_HOOK_Y_DEEP_MIN:    f32 = -(VH * 9.0);
pub const SPACE_HOOK_Y_DEEP_MAX:    f32 = -(VH * 5.0);
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
pub const SPACE_COIN_SCORE:    u32 = 1000;
pub const SPACE_CATCOIN_SCORE:      u32 = SPACE_COIN_SCORE * 2;
pub const SPACE_CATCOIN_BLUE_SCORE: u32 = SPACE_COIN_SCORE * 5;
pub const SPACE_CATCOIN_RED_SCORE:  u32 = SPACE_COIN_SCORE * 25;
pub const SPACE_CATCOIN_BLUE_CHANCE: f32 = 0.22;
pub const SPACE_CATCOIN_RED_CHANCE:  f32 = 0.08;
pub const SPACE_COIN_ANIM_FPS: f32 = 12.0;
pub const SPACE_COIN_R:        f32 = 27.0;
pub const SPACE_COIN_FORMATION_COUNT: usize = 4;
pub const SPACE_COIN_FORMATION_SPACING: f32 = 210.0;
pub const SPACE_COIN_FORMATION_ARC_RISE: f32 = 62.0;
pub const SPACE_COIN_FORMATION_Y_MIN: f32 = -(VH * 3.6);
pub const SPACE_COIN_FORMATION_Y_MAX: f32 = -(VH * 0.95);
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
pub const SPACE_ASTEROID_Y_NEAR_MIN:     f32 = -450.0;  // small, closest to action
pub const SPACE_ASTEROID_Y_NEAR_MAX:     f32 = -80.0;
pub const SPACE_ASTEROID_Y_FAR_MIN:      f32 = -2200.0; // large, highest (visible zoomed-out)
pub const SPACE_ASTEROID_Y_FAR_MAX:      f32 = -700.0;
pub const SPACE_ASTEROID_SIZE_MIN:       f32 = 180.0;
pub const SPACE_ASTEROID_SIZE_MAX:       f32 = 420.0;
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

// Stasis orbit (shared between entry/exit stasis and game-start stasis)
pub const STASIS_ORBIT_R:     f32 = 240.0;
pub const STASIS_ORBIT_OMEGA: f32 = 0.038;
