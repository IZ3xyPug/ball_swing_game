#![allow(dead_code)]
// ── Virtual resolution ────────────────────────────────────────────────────────
pub const VW: f32 = 3840.0;
pub const VH: f32 = 2160.0;

// ── Physics ───────────────────────────────────────────────────────────────────
pub const GRAVITY:        f32 = 0.9;
pub const SWING_TENSION:  f32 = 1.08;
pub const MOMENTUM_CAP:   f32 = 56.0;
pub const ROPE_LEN_MIN:   f32 = 200.0;
pub const ROPE_LEN_MAX:   f32 = 600.0;
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
pub const ROPE_THICKNESS: f32 = 10.0;

// ── Generation ────────────────────────────────────────────────────────────────
pub const GEN_AHEAD:      f32 = VW * 2.5;
pub const HOOKS_SPAWN_BUDGET_PER_TICK:    usize = 2;
pub const PADS_SPAWN_BUDGET_PER_TICK:     usize = 2;
pub const SPINNERS_SPAWN_BUDGET_PER_TICK: usize = 2;
pub const FLIPS_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const ZERO_G_SPAWN_BUDGET_PER_TICK:   usize = 1;
pub const GATES_SPAWN_BUDGET_PER_TICK:    usize = 1;
pub const COIN_BATCHES_BUDGET_PER_TICK:   usize = 1;
pub const MAX_HOOKS_LIVE: usize = 10;
pub const HOOK_POOL_SIZE: usize = 68;
pub const PAD_POOL_SIZE:  usize = 32;
pub const PAD_GAP_MIN:    f32 = 1200.0;
pub const PAD_GAP_MAX:    f32 = 2800.0;
pub const PAD_W:          f32 = 750.0;
pub const PAD_H:          f32 = 125.0;
pub const PAD_BOUNCE_VY_START:  f32 = -46.0;
pub const PAD_BOUNCE_VERTICAL_BOOST: f32 = 1.15;
pub const PAD_BOUNCE_DECAY:     f32 = 0.20;
pub const PAD_BOUNCE_MIN_FACTOR:f32 = 0.30;
pub const PAD_MOVE_RANGE: f32 = 250.0;
pub const PAD_MOVE_SPEED: f32 = 3.0;

pub fn pad_corner_radius() -> f32 {
	(PAD_H * 0.48).clamp(1.0, PAD_H * 0.5 - 1.0)
}
pub const SPINNER_POOL_SIZE: usize = 14;
pub const SPINNER_GAP_MIN:   f32 = 3900.0;
pub const SPINNER_GAP_MAX:   f32 = 6400.0;
pub const SPINNER_W:         f32 = 620.0;
pub const SPINNER_H:         f32 = 70.0;
pub const SPINNER_ROT_SPEED: f32 = 6.4;
pub const HOOK_SPINNER_MIN_X_GAP: f32 = 460.0;
pub const HOOK_SPINNER_PUSH_X:    f32 = 150.0;
pub const SPINNER_BLACK_MOVE_AMP_MIN: f32 = 120.0;
pub const SPINNER_BLACK_MOVE_AMP_MAX: f32 = 260.0;
pub const SPINNER_BLACK_MOVE_SPEED_MIN: f32 = 1.1;
pub const SPINNER_BLACK_MOVE_SPEED_MAX: f32 = 2.1;
pub const ZONE_DISTANCE_STEP:f32 = 20000.0;
pub const START_ZONE_SPINNER_MULT:f32 = 0.50;
pub const PURPLE_ZONE_SPINNER_MULT:f32 = 1.00;
pub const BLACK_ZONE_SPINNER_MULT:f32 = 1.50;
pub const SPINNER_HIT_PUSH_X:f32 = 11.0;
pub const SPINNER_HIT_PUSH_Y:f32 = -28.0;
pub const COIN_POOL_SIZE:    usize = 30;
pub const COIN_GAP_MIN:      f32 = 1200.0;
pub const COIN_GAP_MAX:      f32 = 2400.0;
pub const COIN_R:            f32 = 48.0;
pub const COIN_SCORE:        u32 = 125;
pub const COIN_ARRAY_COUNT:  usize = 5;
pub const COIN_ARRAY_SPACING:f32 = 120.0;
pub const COIN_CURVE_RISE:   f32 = 60.0;
pub const COIN_ARRAY_CHANCE: f32 = 0.28;
pub const COIN_ARRAY_HOOK_DX:f32 = 600.0;
pub const COIN_ARRAY_HOOK_DY:f32 = -742.0;
pub const COIN_ARRAY_Y_MIN:  f32 = -200.0;
pub const COIN_ARRAY_Y_MAX:  f32 = -35.0;
pub const COIN_SINGLE_Y_MIN: f32 = -35.0;
pub const COIN_SINGLE_Y_MAX: f32 = 1650.0;
pub const COIN_MAGNET_RADIUS:f32 = 180.0;
pub const COIN_MAGNET_PULL:  f32 = 0.37;
pub const FLIP_POOL_SIZE:    usize = 16;
pub const FLIP_GAP_MIN:      f32 = 7000.0;
pub const FLIP_GAP_MAX:      f32 = 12000.0;
pub const FLIP_W:            f32 = 110.0;
pub const FLIP_H:            f32 = 110.0;
pub const FLIP_DURATION:     u32 = 300;  // 5 seconds at 60fps
pub const SCORE_X2_POOL_SIZE: usize = 16;
pub const SCORE_X2_GAP_MIN:   f32 = 5600.0;
pub const SCORE_X2_GAP_MAX:   f32 = 9800.0;
pub const SCORE_X2_W:         f32 = 160.0;
pub const SCORE_X2_H:         f32 = 160.0;
pub const SCORE_X2_DURATION:  u32 = 600; // 10 seconds at 60fps
pub const ZERO_G_POOL_SIZE:   usize = 14;
pub const ZERO_G_GAP_MIN:     f32 = 6200.0;
pub const ZERO_G_GAP_MAX:     f32 = 9800.0;
pub const ZERO_G_W:           f32 = 120.0;
pub const ZERO_G_H:           f32 = 120.0;
pub const ZERO_G_DURATION:    u32 = 480; // 8 seconds at 60fps
pub const ZERO_G_GRAVITY_SCALE: f32 = 0.55;
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
pub const ASSET_ASTEROID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/asteroid.webp");

// ── Gravity wells ─────────────────────────────────────────────────────────────
pub const GWELL_POOL_SIZE:     usize = 10;
pub const GWELL_GAP_MIN:       f32 = 5400.0;
pub const GWELL_GAP_MAX:       f32 = 9200.0;
pub const GWELL_RADIUS_MIN:    f32 = 540.0;
pub const GWELL_RADIUS_MAX:    f32 = 1080.0;
pub const GWELL_STRENGTH_MIN:  f32 = 0.75;
pub const GWELL_STRENGTH_MAX:  f32 = 1.0;
pub const GWELL_ON_TICKS:      u32 = 240;   // 4 seconds at 60fps
pub const GWELL_OFF_TICKS:     u32 = 180;   // 3 seconds at 60fps
pub const GWELL_Y_MIN:         f32 = VH * 0.15;
pub const GWELL_Y_MAX:         f32 = VH * 0.80;
pub const GWELL_SPAWN_BUDGET:  usize = 1;
pub const GWELL_VISUAL_SCALE_MIN: f32 = 3.0;   // smallest well = 3× player diameter
pub const GWELL_VISUAL_SCALE_MAX: f32 = 10.0;  // largest well = 10× player diameter
pub const GWELL_RING_COUNT:    u32 = 5;         // concentric alpha rings
pub const GWELL_PULSE_MIN:     f32 = 0.7;
pub const GWELL_PULSE_SPEED:   f32 = 0.08;
pub const GWELL_DISCONNECT_FRAC: f32 = 0.5;  // disconnect from grab at 50% of planet_radius
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
pub const TURRET_SPAWN_BUDGET:  usize = 1;
pub const TURRET_Y_MIN:         f32 = VH * 0.12;
pub const TURRET_Y_MAX:         f32 = VH * 0.80;
pub const TURRET_DETECT_RADIUS: f32 = 2800.0;
pub const BULLET_POOL_SIZE:     usize = 32;
pub const BULLET_W:             f32 = 36.0;
pub const BULLET_H:             f32 = 12.0;
pub const BULLET_SPEED:         f32 = 28.0;
pub const BULLET_LIFETIME_TICKS:u32 = 300; // 5 seconds at 60fps
pub const C_TURRET_BODY:        (u8,u8,u8) = (100, 100, 130);
pub const C_TURRET_BARREL:      (u8,u8,u8) = (70, 70, 90);
pub const C_TURRET_BULLET:      (u8,u8,u8) = (220, 40, 40);

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
pub const SPACE_ENTRY_Y:           f32 = -(VH * 0.35);
/// Depth at which the entry catch planet is centered and momentum is zeroed.
/// Must be below (more negative than) SPACE_ENTRY_Y by enough that the player
/// reaches it while still moving upward. Planet radius + gravity_influence_mult
/// together ensure gravity pulls from here all the way back to SPACE_ENTRY_Y.
pub const SPACE_SETTLE_Y:          f32 = -(VH * 1.1);  // ~-2376 at VH=2160
/// Player py rising back above this (less negative) while in space triggers return.
pub const SPACE_RETURN_Y:          f32 = -(VH * 0.05);
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
pub const SPACE_PLANET_POOL_SIZE:    usize = 12;
pub const SPACE_HOOK_POOL_SIZE:      usize = 64;
pub const SPACE_COIN_POOL_SIZE:      usize = 40;
pub const SPACE_BLACKHOLE_POOL_SIZE: usize = 8;

// Space object spawn budgets per tick
pub const SPACE_PLANET_SPAWN_BUDGET:    usize = 1;
pub const SPACE_HOOK_SPAWN_BUDGET:      usize = 3;  // one per Y-band per spawn tick
pub const SPACE_COIN_SPAWN_BUDGET:      usize = 2;
pub const SPACE_BLACKHOLE_SPAWN_BUDGET: usize = 1;

// Space planet parameters
pub const SPACE_PLANET_GAP_MIN:         f32 = 2400.0;
pub const SPACE_PLANET_GAP_MAX:         f32 = 5200.0;
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

// Space coin parameters
pub const SPACE_COIN_GAP_MIN:  f32 = 580.0;
pub const SPACE_COIN_GAP_MAX:  f32 = 1200.0;
pub const SPACE_COIN_SCORE:    u32 = 1000;
pub const SPACE_COIN_R:        f32 = 56.0;

// Black hole parameters
pub const SPACE_BLACKHOLE_GAP_MIN:       f32 = 5000.0;
pub const SPACE_BLACKHOLE_GAP_MAX:       f32 = 9000.0;
pub const SPACE_BLACKHOLE_RADIUS_MIN:    f32 = 100.0;
pub const SPACE_BLACKHOLE_RADIUS_MAX:    f32 = 200.0;
pub const SPACE_BLACKHOLE_GRAV_STRENGTH: f32 = 0.7;
pub const SPACE_BLACKHOLE_Y_MIN:         f32 = -(VH * 2.8);
pub const SPACE_BLACKHOLE_Y_MAX:         f32 = -(VH * 0.55);

// Camera behavior during space transition
pub const SPACE_CAM_LERP_IN:    f32 = 0.048;  // slower lerp (dramatic ascent)
pub const SPACE_CAM_ZOOM_IN:    f32 = 1.05;   // slight zoom toward player in space
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
pub const C_SPACE_HOOK:  (u8,u8,u8) = (155, 115, 255);
pub const C_SPACE_HOOK_ON: (u8,u8,u8) = (210, 185, 255);
pub const C_BLACKHOLE:   (u8,u8,u8) = (18,  8,   26);

// Oxygen HUD bar
pub const OXYGEN_BAR_W:  f32 = 700.0;
pub const OXYGEN_BAR_H:  f32 = 42.0;
pub const C_OXY_FULL:    (u8,u8,u8) = (80,  220, 160);
pub const C_OXY_MID:     (u8,u8,u8) = (240, 200, 55);
pub const C_OXY_LOW:     (u8,u8,u8) = (220, 55,  55);
