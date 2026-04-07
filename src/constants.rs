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
pub const RELEASE_MIN_SWING_SPEED: f32 = 3.2;
pub const RELEASE_SURGE_SCALE: f32 = 0.34;
pub const RELEASE_SURGE_MAX: f32 = 11.0;
pub const BOOST_CHARGE_PER_PICKUP: f32 = 0.22;
pub const BOOST_USE_MIN: f32 = 0.15;

// ── Object sizes ──────────────────────────────────────────────────────────────
pub const PLAYER_R:       f32 = 40.0;
pub const HOOK_R:         f32 = 38.0;
pub const ROPE_THICKNESS: f32 = 10.0;

// ── Generation ────────────────────────────────────────────────────────────────
pub const GEN_AHEAD:      f32 = VW * 2.5;
pub const MAX_HOOKS_LIVE: usize = 10;
pub const HOOK_POOL_SIZE: usize = 68;
pub const PAD_POOL_SIZE:  usize = 32;
pub const PAD_GAP_MIN:    f32 = 1200.0;
pub const PAD_GAP_MAX:    f32 = 2800.0;
pub const PAD_W:          f32 = 750.0;
pub const PAD_H:          f32 = 125.0;
pub const PAD_BOUNCE_VY_START:  f32 = -46.0;
pub const PAD_BOUNCE_DECAY:     f32 = 0.20;
pub const PAD_BOUNCE_MIN_FACTOR:f32 = 0.30;
pub const PAD_MOVE_RANGE: f32 = 250.0;
pub const PAD_MOVE_SPEED: f32 = 3.0;
pub const SPINNER_POOL_SIZE: usize = 14;
pub const SPINNER_GAP_MIN:   f32 = 3700.0;
pub const SPINNER_GAP_MAX:   f32 = 6600.0;
pub const SPINNER_W:         f32 = 620.0;
pub const SPINNER_H:         f32 = 70.0;
pub const SPINNER_ROT_SPEED: f32 = 6.4;
pub const SPINNER_HIT_PUSH_X:f32 = 11.0;
pub const SPINNER_HIT_PUSH_Y:f32 = -28.0;
pub const BOOST_POOL_SIZE:   usize = 48;
pub const BOOST_GAP_MIN:     f32 = 1700.0;
pub const BOOST_GAP_MAX:     f32 = 3400.0;
pub const BOOST_W:           f32 = 92.0;
pub const BOOST_H:           f32 = 92.0;
pub const BOOST_VX:          f32 = 3.6;
pub const BOOST_VY:          f32 = -1.4;
pub const COIN_POOL_SIZE:    usize = 30;
pub const COIN_GAP_MIN:      f32 = 1200.0;
pub const COIN_GAP_MAX:      f32 = 2400.0;
pub const COIN_R:            f32 = 48.0;
pub const COIN_SCORE:        u32 = 125;
pub const COIN_ARRAY_COUNT:  usize = 5;
pub const COIN_ARRAY_SPACING:f32 = 120.0;
pub const COIN_CURVE_RISE:   f32 = 60.0;
pub const COIN_ARRAY_CHANCE: f32 = 0.28;
pub const COIN_ARRAY_Y_MIN:  f32 = -440.0;
pub const COIN_ARRAY_Y_MAX:  f32 = -100.0;
pub const COIN_SINGLE_Y_MIN: f32 = -35.0;
pub const COIN_SINGLE_Y_MAX: f32 = 1650.0;
pub const COIN_MAGNET_RADIUS:f32 = 165.0;
pub const COIN_MAGNET_PULL:  f32 = 0.37;
pub const FLIP_POOL_SIZE:    usize = 16;
pub const FLIP_GAP_MIN:      f32 = 7000.0;
pub const FLIP_GAP_MAX:      f32 = 12000.0;
pub const FLIP_W:            f32 = 110.0;
pub const FLIP_H:            f32 = 110.0;
pub const FLIP_DURATION:     u32 = 600;  // 10 seconds at 60fps
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
pub const ZOOM_TOP_MARGIN:  f32 = VH * 0.06;
pub const ZOOM_MAX:         f32 = 1.8;
pub const ZOOM_OUT_LERP:    f32 = 0.22;
pub const ZOOM_IN_LERP:     f32 = 0.05;
pub const ZOOM_LOOKAHEAD_T: f32 = 12.0;

// ── Colours ───────────────────────────────────────────────────────────────────
pub const C_SKY_TOP:  (u8,u8,u8) = (15,  20,  45 );
pub const C_SKY_BOT:  (u8,u8,u8) = (30,  50,  90 );
pub const C_PLAYER:   (u8,u8,u8) = (80,  220, 160);
pub const C_HOOK:     (u8,u8,u8) = (200, 60,  20 );
pub const C_HOOK_ON:  (u8,u8,u8) = (255, 90,  70 );
pub const C_HOOK_NEAR:(u8,u8,u8) = (255, 120, 50 );
pub const C_ROPE:     (u8,u8,u8) = (220, 220, 220);
pub const C_DANGER:   (u8,u8,u8) = (200, 50,  50 );
pub const C_PAD:      (u8,u8,u8) = (60,  200, 255);
pub const C_PAD_HIT:  (u8,u8,u8) = (160, 255, 255);
pub const C_SPINNER:  (u8,u8,u8) = (255, 100, 95);
pub const C_BOOST:    (u8,u8,u8) = (120, 255, 140);
pub const C_COIN:     (u8,u8,u8) = (255, 95, 210);
pub const C_FLIP:     (u8,u8,u8) = (255, 245, 120);

// ── Spawn positions ───────────────────────────────────────────────────────────
pub const SPAWN_X: f32 = VW * 0.22;
pub const SPAWN_Y: f32 = VH * 0.38;
pub const START_HOOK_X: f32 = SPAWN_X + 160.0;
pub const START_HOOK_Y: f32 = SPAWN_Y - 420.0;
