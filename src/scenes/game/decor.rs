//! decor.rs — distant scenery drifting behind the swing.
//!
//! The normal-zone backdrop is a screen-fixed sky: over a long Casual or
//! Normal stretch nothing in it moved, which is most of why minute forty
//! looked like minute four. These are planets, a moon, a galaxy, satellites
//! and wrecks — and, rarely, an oddity: a UFO, a space whale, a drifting
//! astronaut, a rubber duck — that slide past with camera parallax.
//!
//! SPARSE by design: a piece every ~25-45 s of travel, never more than
//! `DECOR_MAX_LIVE` at once, dimmed and behind everything, so it reads as
//! "the world is big and you are moving through it" and never as clutter
//! competing with the nodes. Art: assets/pixellab/decor (PixelLab,
//! provenance in SOURCES.md).

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// One kind of scenery.
pub struct DecorKind {
    pub name: &'static str,
    pub png: &'static [u8],
    /// On-screen size range, virtual px.
    pub size: (f32, f32),
    /// Screen px moved per world px the camera moves: small is far.
    pub parallax: f32,
    /// Relative chance of being picked.
    pub weight: u32,
    /// Its own motion: screen px per tick, and degrees per tick of spin.
    pub drift: (f32, f32),
    pub spin: f32,
    /// A resting tilt (degrees), for things drawn pointing up.
    pub tilt: f32,
    /// Bobs gently up and down.
    pub bob: bool,
    /// An oddity: rare, and reported when seen (achievement hook).
    pub rare: bool,
    /// Multiplied into the art: dims and fades it into the distance.
    pub tint: (u8, u8, u8, u8),
}

macro_rules! png {
    ($n:literal) => {
        include_bytes!(concat!("../../../assets/pixellab/decor/", $n, ".png"))
    };
}

// Atmospheric perspective: the further, the darker, bluer and fainter —
// full-strength art sat in the node band and competed with the nodes.
const FAR: (u8, u8, u8, u8) = (118, 124, 158, 175);
const MID: (u8, u8, u8, u8) = (150, 156, 186, 200);
const NEAR: (u8, u8, u8, u8) = (182, 186, 210, 220);

pub const DECOR_KINDS: &[DecorKind] = &[
    // Planets: big, far, slow.
    DecorKind { name: "ringed gas giant", png: png!("planet_ringed"), size: (620.0, 900.0), parallax: 0.09, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "ice world", png: png!("planet_ice"), size: (420.0, 640.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "desert world", png: png!("planet_desert"), size: (420.0, 640.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "ocean world", png: png!("planet_ocean"), size: (460.0, 700.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "violet giant", png: png!("planet_violet"), size: (560.0, 820.0), parallax: 0.08, weight: 9, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "lava world", png: png!("planet_lava"), size: (380.0, 560.0), parallax: 0.11, weight: 8, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "cratered moon", png: png!("moon_cratered"), size: (300.0, 460.0), parallax: 0.13, weight: 2, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    // Middle distance.
    DecorKind { name: "small ringed planet", png: png!("ringed_small"), size: (200.0, 300.0), parallax: 0.16, weight: 7, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: MID },
    // Floaters: they cross the sky under their own drift (see
    // `self_propelled`), never pushed around by the camera.
    DecorKind { name: "ring station", png: png!("station"), size: (240.0, 340.0), parallax: 0.18, weight: 6, drift: (-0.9, 0.0), spin: 0.05, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "satellite", png: png!("satellite"), size: (170.0, 240.0), parallax: 0.22, weight: 7, drift: (-1.3, 0.06), spin: 0.08, tilt: 0.0, bob: false, rare: false, tint: NEAR },
    DecorKind { name: "derelict", png: png!("derelict"), size: (220.0, 320.0), parallax: 0.2, weight: 5, drift: (-1.0, 0.1), spin: 0.03, tilt: -8.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "asteroids", png: png!("asteroids"), size: (200.0, 300.0), parallax: 0.2, weight: 7, drift: (-1.1, 0.04), spin: 0.06, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "crystals", png: png!("crystals"), size: (170.0, 240.0), parallax: 0.2, weight: 4, drift: (-0.8, 0.0), spin: 0.04, tilt: 0.0, bob: true, rare: false, tint: NEAR },
    DecorKind { name: "space buoy", png: png!("buoy"), size: (130.0, 180.0), parallax: 0.24, weight: 3, drift: (-0.7, 0.0), spin: 0.0, tilt: 0.0, bob: true, rare: false, tint: NEAR },
    // Movers: across the sky on their own.
    DecorKind { name: "comet", png: png!("comet"), size: (220.0, 320.0), parallax: 0.15, weight: 5, drift: (7.5, 1.6), spin: 0.0, tilt: 12.0, bob: false, rare: false, tint: NEAR },
    DecorKind { name: "rocket", png: png!("rocket"), size: (150.0, 210.0), parallax: 0.2, weight: 3, drift: (-3.0, -3.2), spin: 0.0, tilt: -43.0, bob: false, rare: false, tint: NEAR },
    // Oddities: rare, and worth pointing out to a friend.
    DecorKind { name: "UFO", png: png!("ufo"), size: (180.0, 240.0), parallax: 0.22, weight: 1, drift: (-2.0, 0.0), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: NEAR },
    DecorKind { name: "space whale", png: png!("space_whale"), size: (520.0, 700.0), parallax: 0.1, weight: 1, drift: (-1.2, -0.1), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: MID },
    DecorKind { name: "astronaut", png: png!("astronaut"), size: (150.0, 200.0), parallax: 0.24, weight: 1, drift: (-0.9, 0.15), spin: 0.35, tilt: 0.0, bob: false, rare: true, tint: NEAR },
    DecorKind { name: "space jellyfish", png: png!("jellyfish"), size: (190.0, 260.0), parallax: 0.2, weight: 1, drift: (-0.7, -0.35), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: NEAR },
    DecorKind { name: "rubber duck", png: png!("rubber_duck"), size: (120.0, 160.0), parallax: 0.26, weight: 1, drift: (-0.9, 0.08), spin: 0.25, tilt: 0.0, bob: false, rare: true, tint: NEAR },
];

/// Planets and the galaxy. They live in the sky, like the nebula: pinned to a
/// spot in it, they zoom out with it (moving and shrinking with the sky, not
/// the world). Across it they only ever advance with the player: their
/// parallax follows the player's forward travel alone, so moving backwards
/// leaves them where they are. Everything else parallaxes off the camera,
/// both ways.
const FORWARD_ONLY: &[&str] = &[
    "ringed gas giant", "ice world", "desert world", "ocean world",
    "violet giant", "lava world", "small ringed planet",
    "cratered moon",
];

/// Everything that is not pinned in the sky (the planets, the moon and the
/// galaxy) moves under its own power: no camera parallax at all, so the
/// player's swinging (up, down, back, forward) never pushes it around. Each
/// is its own object crossing the sky at its own drift. Purely scenery —
/// the comet ATTACK is a separate system.
fn self_propelled(k: &DecorKind) -> bool { !forward_only(k) }

/// How many times further the player must travel to move a planet or the
/// galaxy the same distance across the screen. At a typical 74 px/tick a
/// planet at parallax 0.10 took ~9 s to cross the screen, far too quick for
/// something meant to be that far away; 3 makes it ~27 s. Applies on top of
/// each kind's own `parallax`, so their relative depths are unchanged.
pub const PLANET_DISTANCE_SCALE: f32 = 3.0;
/// The one parallax every planet and the galaxy drift at (before
/// `PLANET_DISTANCE_SCALE`). Their kinds' own parallax made the small ones
/// cross faster than the big ones; all the same now, a bit quicker than the
/// old average (~0.10).
pub const PLANET_PARALLAX: f32 = 0.12;
/// Planets and the galaxy are drawn at this share of their kind's size, so
/// they read as farther away.
pub const PLANET_SIZE_SCALE: f32 = 0.75;
/// Screen px (at normal zoom) a new planet keeps from the last one. Spawning
/// is paced by travel, but planets drift so slowly that the last one has
/// barely moved by the next spawn, so this holds a new one back until the
/// last is well clear of the right edge.
pub const PLANET_SPACING: f32 = VW * 0.6;
/// Planets and the galaxy spawn at this world height or higher (world y, up
/// is negative; the HUD's height readout). Low down that height is further
/// above the screen than the sky's zoom-out ever reveals, so a planet only
/// spawns once the player is high enough (the camera zoomed out enough) to
/// see it; until then the spawner waits and rolls again.
pub const PLANET_MAX_SPAWN_Y: f32 = -1_000.0;

// ── Depth ─────────────────────────────────────────────────────────────────────
// Each planet gets a depth when it spawns, 0 (the near layer) to 1 (the far
// layer). Farther is smaller, slower, dimmer, and barely shrinks as the sky
// zooms out (the farthest things change least). Mixed depths make the sky
// deep; one planet's speed still matches its size, so smaller ones never
// outrun bigger ones. A rare near pass (negative depth) is big, crisp and
// quick, and the rest read as far by contrast.
/// At depth 1: size kept, speed kept, brightness kept, and the share of the
/// sky's zoom-out taken. Depth 0 keeps all of them at 1, except speed.
pub const PLANET_FAR_SIZE: f32 = 0.55;
pub const PLANET_FAR_SPEED: f32 = 0.5;
pub const PLANET_NEAR_SPEED: f32 = 1.15;
pub const PLANET_FAR_BRIGHTNESS: f32 = 0.72;
pub const PLANET_FAR_ZOOM: f32 = 0.35;
/// The share of planet spawns that are a near pass, and how much bigger and
/// faster one is. Off: planets are only ever far away.
pub const NEAR_PASS_CHANCE: f32 = 0.0;
/// The nearest depth a planet may roll: only the far half of the range, so
/// every planet reads as distant.
pub const PLANET_MIN_DEPTH: f32 = 0.5;
pub const NEAR_PASS_SIZE: f32 = 1.7;
pub const NEAR_PASS_SPEED: f32 = 3.0;
/// Planets at least this deep draw behind the moving stars (they pass in
/// front, as nearer things do); nearer ones and near passes draw in front.
pub const PLANET_BEHIND_STARS_DEPTH: f32 = 0.4;

// ── Height ────────────────────────────────────────────────────────────────────
// Looking toward the horizon means looking through more air: planets low on
// the screen (near the horizon glow) are dimmer, and they clear as the player
// climbs (the sky zoomed out).
/// Brightness kept at the horizon band's top edge, and where that band runs
/// (screen y, as shares of the screen's height).
pub const PLANET_HORIZON_BRIGHTNESS: f32 = 0.6;
pub const PLANET_HORIZON_BAND: (f32, f32) = (0.35, 0.75);
/// Brightness at normal zoom; full brightness when the sky is fully zoomed out.
pub const PLANET_LOW_BRIGHTNESS: f32 = 0.85;

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

/// A new planet's depth, and its size sized to it: now and then a near pass,
/// otherwise anywhere from the near layer to the far one.
fn roll_depth(seed: &mut u64, size: f32) -> (f32, f32) {
    if lcg(seed) < NEAR_PASS_CHANCE {
        (-1.0, size * NEAR_PASS_SIZE)
    } else {
        let depth = lerp(PLANET_MIN_DEPTH, 1.0, lcg(seed));
        (depth, size * lerp(1.0, PLANET_FAR_SIZE, depth))
    }
}

/// Dev key: a random planet (or the galaxy) at a random depth, straight onto
/// the right side of the screen, ignoring the height and spacing rules so the
/// look can be tried anywhere. Takes a free slot, or the first one if none is.
pub fn debug_spawn_planet(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let flipped = st.lock().unwrap().gravity_dir < 0.0;
    let Some(sky) = super::background::sky_frame(c, flipped) else { return };
    let mut s = st.lock().unwrap();
    if s.in_space_mode || s.boss_active { return; }
    if s.decor.len() < DECOR_POOL { s.decor.resize(DECOR_POOL, DecorSlot::default()); }
    let mut seed = s.seed ^ (s.decor_ticks as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5;
    let planets: Vec<usize> = (0..DECOR_KINDS.len()).filter(|&i| forward_only(&DECOR_KINDS[i])).collect();
    let kind = planets[(lcg(&mut seed) * planets.len() as f32) as usize % planets.len()];
    let k = &DECOR_KINDS[kind];
    let size = lcg_range(&mut seed, k.size.0, k.size.1) * PLANET_SIZE_SCALE;
    let (depth, size) = roll_depth(&mut seed, size);
    let (x, y) = (VW * 0.72, lcg_range(&mut seed, DECOR_Y.0, DECOR_Y.1));
    let (sky_u, sky_v) = sky.to_sky(x, y);
    let slot = s.decor.iter().position(|d| !d.active).unwrap_or(0);
    s.decor[slot] = DecorSlot { active: true, kind, x, y, sky_u, sky_v, depth, size, rot: k.tilt, phase: lcg(&mut seed) * 6.283, seen: false };
    s.seed = seed;
    log::info!("debug planet: {} depth {:.2}{}", k.name, depth, if depth < 0.0 { " (near pass)" } else if depth >= PLANET_BEHIND_STARS_DEPTH { " (behind the moving stars)" } else { "" });
}

/// Speed, sky-zoom share and brightness for a planet's depth.
fn depth_look(depth: f32) -> (f32, f32, f32) {
    if depth < 0.0 { return (NEAR_PASS_SPEED, 1.0, 1.0); }
    (lerp(PLANET_NEAR_SPEED, PLANET_FAR_SPEED, depth), lerp(1.0, PLANET_FAR_ZOOM, depth), lerp(1.0, PLANET_FAR_BRIGHTNESS, depth))
}

/// Brightness for where a planet is on screen and how zoomed out the sky is.
fn height_brightness(screen_y: f32, sky_scale: f32) -> f32 {
    let horizon = smooth(PLANET_HORIZON_BAND.0 * VH, PLANET_HORIZON_BAND.1 * VH, screen_y);
    let s_min = sky_scale_min();
    let climbed = ((1.0 - sky_scale) / (1.0 - s_min).max(1e-3)).clamp(0.0, 1.0);
    lerp(1.0, PLANET_HORIZON_BRIGHTNESS, horizon) * lerp(PLANET_LOW_BRIGHTNESS, 1.0, climbed)
}

fn sky_scale_min() -> f32 { sky_scale(SKY_ZOOM_FLOOR) }

// ── Atmospheric fade ──────────────────────────────────────────────────────────
// Far things read as far because they are washed toward the colour of the sky
// they sit in and lose contrast. A tint alone can only darken (it multiplies),
// and alpha would let the stars show through a solid planet, so the fade is
// baked into each planet's image once, when it is first drawn at a size.
/// The upper sky's haze the planets fade toward (a touch above its base
/// colour, so darks lift rather than vanish).
pub const PLANET_HAZE_RGB: [f32; 3] = [30.0, 30.0, 58.0];
/// How far toward the haze: 0 untouched, 1 all haze.
pub const PLANET_HAZE: f32 = 0.35;
/// Contrast kept, about the planet's own mean brightness.
pub const PLANET_CONTRAST: f32 = 0.75;
/// Colour kept (1 = full saturation).
pub const PLANET_SATURATION: f32 = 0.7;
/// Brightness kept before the haze blend. At night distance sinks things
/// toward the dark sky; without this the fade would lift them brighter than
/// the old flat tint (which kept ~half) and they would read as nearer.
pub const PLANET_BRIGHTNESS: f32 = 0.55;

/// One pixel through the fade: contrast about `mean`, desaturate, then blend
/// toward the haze. Alpha is untouched, so the planet stays solid.
pub fn haze_pixel(p: [f32; 3], mean: f32) -> [f32; 3] {
    let lum = 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
    let mut out = [0.0f32; 3];
    for i in 0..3 {
        let c = lum + (p[i] - lum) * PLANET_SATURATION;
        let c = (mean + (c - mean) * PLANET_CONTRAST) * PLANET_BRIGHTNESS;
        out[i] = c * (1.0 - PLANET_HAZE) + PLANET_HAZE_RGB[i] * PLANET_HAZE;
    }
    out
}

/// Softening: blur radius per px of drawn size (the art is 128 px, drawn
/// ~3x, so ~1 art pixel), so edges are less crisp than the nodes'.
pub const PLANET_SOFTEN: f32 = 0.007;
/// The light the rim and glow come from (upper left, as the nebula's), and
/// their colour: a cool atmosphere blue.
pub const PLANET_LIGHT_DIR: (f32, f32) = (-0.6, -0.8);
pub const PLANET_RIM_RGB: [f32; 3] = [150.0, 175.0, 255.0];
/// Rim strength on the lit edge, and the outer glow's alpha and reach (a
/// share of the drawn size).
pub const PLANET_RIM: f32 = 0.55;
pub const PLANET_GLOW: f32 = 0.35;
pub const PLANET_GLOW_REACH: f32 = 0.035;
fn smooth(e0: f32, e1: f32, x: f32) -> f32 { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// Box blur of a w x h float field, `passes` times each way.
fn blur(m: &mut [f32], w: usize, h: usize, r: usize, passes: usize) {
    if r == 0 { return; }
    let mut tmp = vec![0.0f32; m.len()];
    for _ in 0..passes {
        for y in 0..h { for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] = m[y * w + lo..=y * w + hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
        }}
        for y in 0..h { for x in 0..w {
            let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
            m[y * w + x] = (lo..=hi).map(|yy| tmp[yy * w + x]).sum::<f32>() / (hi - lo + 1) as f32;
        }}
    }
}

/// A planet's image at `px` as a far body (see `build_far_planet`), or `None`
/// while it is still being built. Building takes a moment (several blurs over
/// a few hundred px), and doing it on the game thread froze the game whenever
/// a planet first appeared, so it happens on a background thread and the
/// planet waits, unseen, a few frames: it spawns above or beside the screen
/// anyway. Cached by (asset, px).
fn far_planet_image(bytes: &'static [u8], px: f32, disc: bool) -> Option<Arc<image::RgbaImage>> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    type Cache = Mutex<HashMap<(usize, u32), Option<Option<Arc<image::RgbaImage>>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (bytes.as_ptr() as usize, px as u32);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut g = cache.lock().ok()?;
    match g.get(&key) {
        // Built (or failed to decode).
        Some(Some(done)) => done.clone(),
        // Being built.
        Some(None) => None,
        None => {
            g.insert(key, None);
            drop(g);
            std::thread::spawn(move || {
                let built = build_far_planet(bytes, px, disc);
                if let Ok(mut g) = cache.lock() { g.insert(key, Some(built)); }
            });
            None
        }
    }
}

/// A planet's image at `px` as a far body: the atmospheric fade
/// (`haze_pixel`), softened, lit into a rim with a faint glow outside (not
/// for the galaxy, which has no disc).
fn build_far_planet(bytes: &'static [u8], px: f32, disc: bool) -> Option<Arc<image::RgbaImage>> {
    crate::scenes::game::helpers::pl_image_cached(bytes, px).map(|src| {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let n = w * h;
        // 1. Atmospheric fade, about the planet's own mean brightness.
        let (mut sum, mut cnt) = (0.0f32, 0.0f32);
        for p in src.pixels() {
            if p[3] > 128 { sum += 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32; cnt += 1.0; }
        }
        let mean = if cnt > 0.0 { sum / cnt } else { 128.0 };
        // Premultiplied, so the blur does not drag dark fringes in from the
        // transparent surround.
        let mut ch: [Vec<f32>; 4] = std::array::from_fn(|_| vec![0.0f32; n]);
        for (i, p) in src.pixels().enumerate() {
            let a = p[3] as f32 / 255.0;
            let q = haze_pixel([p[0] as f32, p[1] as f32, p[2] as f32], mean);
            for c in 0..3 { ch[c][i] = q[c] * a; }
            ch[3][i] = a;
        }
        // 2. Soften.
        let r = (px * PLANET_SOFTEN).round() as usize;
        for m in ch.iter_mut() { blur(m, w, h, r, 2); }
        let alpha = ch[3].clone();
        // 3. Rim light on the lit edge, and a glow just outside the disc.
        if disc {
            let (lx, ly) = { let l = (PLANET_LIGHT_DIR.0.powi(2) + PLANET_LIGHT_DIR.1.powi(2)).sqrt(); (PLANET_LIGHT_DIR.0 / l, PLANET_LIGHT_DIR.1 / l) };
            // A smoothed alpha gives the edge's outward normal.
            let mut sm = alpha.clone();
            blur(&mut sm, w, h, ((px * 0.012).round() as usize).max(1), 2);
            let mut glow = alpha.clone();
            blur(&mut glow, w, h, ((px * PLANET_GLOW_REACH).round() as usize).max(1), 3);
            for y in 1..h - 1 { for x in 1..w - 1 {
                let i = y * w + x;
                let (gx, gy) = (sm[i + 1] - sm[i - 1], sm[i + w] - sm[i - w]);
                let g = (gx * gx + gy * gy).sqrt();
                // Outward normal is down the alpha slope.
                let lit = if g > 1e-4 { ((-gx * lx - gy * ly) / g).max(0.0) } else { 0.0 };
                let edge = (g * px * 0.02).min(1.0);
                let rim = PLANET_RIM * edge * lit * alpha[i];
                for c in 0..3 { ch[c][i] += PLANET_RIM_RGB[c] * rim; }
                // The glow sits behind: only where the planet is not solid,
                // and faded out before the image's own edge.
                let border = smooth(0.0, 0.06, (x as f32 / w as f32).min(1.0 - x as f32 / w as f32))
                    * smooth(0.0, 0.06, (y as f32 / h as f32).min(1.0 - y as f32 / h as f32));
                let ga = ((glow[i] - alpha[i]).max(0.0) * 2.0).min(1.0) * PLANET_GLOW * (0.35 + 0.65 * lit.max(0.3)) * border;
                let a = alpha[i];
                let add = ga * (1.0 - a);
                for c in 0..3 { ch[c][i] += PLANET_RIM_RGB[c] * 0.55 * add; }
                ch[3][i] = a + add;
            }}
        }
        let mut img = image::RgbaImage::new(w as u32, h as u32);
        for (i, p) in img.pixels_mut().enumerate() {
            let a = ch[3][i].clamp(0.0, 1.0);
            if a <= 0.0 { continue; }
            let rgb = [0, 1, 2].map(|c| (ch[c][i] / a).clamp(0.0, 255.0) as u8);
            *p = image::Rgba([rgb[0], rgb[1], rgb[2], (a * 255.0) as u8]);
        }
        Arc::new(img)
    })
}

fn forward_only(k: &DecorKind) -> bool {
    FORWARD_ONLY.contains(&k.name)
}

/// Pooled decor objects, and the most that may be live at once.
pub const DECOR_POOL: usize = 4;
pub const DECOR_MAX_LIVE: usize = 3;
/// World px of camera travel between pieces, and before the first one.
pub const DECOR_GAP: (f32, f32) = (7_500.0, 13_500.0);
pub const DECOR_FIRST_AT: f32 = 1_800.0;
/// The band of sky decor may occupy (screen y of its centre): above the
/// cloud tops at the bottom of the backdrop, below the HUD.
pub const DECOR_Y: (f32, f32) = (260.0, 1_150.0);

pub fn decor_id(i: usize) -> String {
    format!("bg_decor_{i}")
}

/// The pooled objects, hidden, for the scene builder. Inserted straight
/// after the backdrop so they draw behind everything else on layer 0.
/// The same slot drawn behind the moving stars (see
/// `PLANET_BEHIND_STARS_DEPTH`): added to the scene before them.
pub fn decor_back_id(i: usize) -> String {
    format!("bg_decor_back_{i}")
}

/// The pooled decor objects, in front of the moving stars; with `back`, their
/// twins behind them.
pub fn decor_objects(ctx: &mut Context, back: bool) -> Vec<(String, GameObject)> {
    (0..DECOR_POOL)
        .map(|i| {
            let id = if back { decor_back_id(i) } else { decor_id(i) };
            let mut o = GameObject::new_rect(
                ctx, id.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (64.0, 64.0), 0.0),
                             image: Arc::new(image::RgbaImage::new(1, 1)), color: None }),
                (64.0, 64.0), (-9000.0, -9000.0), vec!["decor".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
            );
            o.ignore_zoom = true;
            o.gravity = 0.0;
            o.visible = false;
            (id, o)
        })
        .collect()
}

/// Weighted pick, never the kind that was picked last, and at most one
/// oddity at a time on screen.
pub fn pick_kind(seed: &mut u64, last: Option<usize>, rare_live: bool) -> usize {
    let ok = |i: usize| Some(i) != last && !(rare_live && DECOR_KINDS[i].rare);
    let total: u32 = (0..DECOR_KINDS.len()).filter(|&i| ok(i)).map(|i| DECOR_KINDS[i].weight).sum();
    let mut r = (lcg(seed) * total.max(1) as f32) as u32;
    for i in 0..DECOR_KINDS.len() {
        if !ok(i) { continue; }
        let w = DECOR_KINDS[i].weight;
        if r < w {
            return i;
        }
        r -= w;
    }
    0
}

/// The camera's y and zoom as the renderer will use them THIS frame.
///
/// The sky (`tick_background`) ticks during `on_update`, before the engine's
/// camera stage, which still advances the zoom lerp (shifting the camera
/// about its zoom anchor) and, without an anchor, the follow lerp. Reading
/// `cam.position`/`cam.zoom` as-is would zoom the sky one camera step behind
/// the world, so this
/// applies that step the way `Camera::advance_zoom_lerp` and `lerp_toward`
/// do. Without an anchor the engine also clamps to the world; that path only
/// runs during the intro recovery and is not mirrored. Screen shake is left
/// out: the engine rolls a fresh random offset in that same stage, so it
/// cannot be predicted, and a far planet not shaking with the world is fine.
pub(super) fn frame_view_y(c: &Canvas) -> (f32, f32) {
    let Some(cam) = c.camera() else { return (0.0, 1.0) };
    let (mut y, mut zoom) = (cam.position.1, cam.zoom);
    if (zoom - cam.zoom_target).abs() > 0.0001 {
        let old = zoom;
        zoom += (cam.zoom_target - zoom) * cam.zoom_lerp_speed;
        if (zoom - cam.zoom_target).abs() < 0.001 {
            zoom = cam.zoom_target;
        }
        let anchor_y = cam.zoom_anchor.map(|a| a.1).unwrap_or(y + VH / (2.0 * old));
        y = anchor_y - (anchor_y - y) * old / zoom;
    }
    if cam.zoom_anchor.is_none() {
        if let Some(p) = c.get_game_object("player") {
            let visible_h = VH / zoom.max(0.01);
            let ty = (p.position.1 + p.size.1 * 0.5 - visible_h * 0.5)
                .clamp(0.0, (cam.world_size.1 - visible_h).max(0.0));
            y += (ty - y) * cam.lerp_speed;
        }
    }
    (y, (zoom + cam.effects.zoom_punch_amount()).max(0.01))
}

pub fn tick_decor(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let cam = c.camera().map(|cam| cam.position).unwrap_or((0.0, 0.0));
    let (hidden, first, flipped) = {
        let s = st.lock().unwrap();
        (s.in_space_mode || s.boss_active || s.dead || s.space_launch_active, s.decor_cam_prev.is_none(),
         s.gravity_dir < 0.0)
    };
    let sky = super::background::sky_frame(c, flipped);
    if hidden {
        let mut s = st.lock().unwrap();
        s.decor_cam_prev = Some(cam);
        s.decor_player_prev_x = Some(s.px);
        let live: Vec<usize> = s.decor.iter().enumerate().filter(|(_, d)| d.active).map(|(i, _)| i).collect();
        for d in s.decor.iter_mut() { d.active = false; }
        drop(s);
        for i in live {
            for id in [decor_id(i), decor_back_id(i)] {
                if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
            }
        }
        return;
    }
    if first {
        let mut s = st.lock().unwrap();
        s.decor_cam_prev = Some(cam);
        s.decor_player_prev_x = Some(s.px);
    }

    let mut s = st.lock().unwrap();
    if s.decor.len() < DECOR_POOL {
        s.decor.resize(DECOR_POOL, DecorSlot::default());
    }
    let prev = s.decor_cam_prev.unwrap_or(cam);
    let (dx, dy) = (cam.0 - prev.0, cam.1 - prev.1);
    s.decor_cam_prev = Some(cam);
    // A camera jump (respawn, warp) is not travel: ignore it.
    let dx = if dx.abs() > 2_000.0 || dy.abs() > 2_000.0 { 0.0 } else { dx };
    if dx > 0.0 {
        s.decor_travel += dx;
    }
    // The player's own forward travel, for the planets. Same jump guard.
    let player_prev = s.decor_player_prev_x.unwrap_or(s.px);
    let player_dx = s.px - player_prev;
    s.decor_player_prev_x = Some(s.px);
    let player_forward = if player_dx.abs() > 2_000.0 { 0.0 } else { player_dx.max(0.0) };
    s.decor_ticks = s.decor_ticks.wrapping_add(1);
    let t = s.decor_ticks as f32;

    // Spawn: just off the right edge, clear of whatever is already up.
    let live = s.decor.iter().filter(|d| d.active).count();
    if s.decor_travel >= s.decor_next_at && live < crate::graphics::decor_max_live(DECOR_MAX_LIVE) {
        if let Some(slot) = s.decor.iter().position(|d| !d.active) {
            let rare_live = s.decor.iter().any(|d| d.active && DECOR_KINDS[d.kind].rare);
            let last = s.decor_last_kind;
            let mut seed = s.seed ^ (s.decor_travel as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let kind = pick_kind(&mut seed, last, rare_live);
            let k = &DECOR_KINDS[kind];
            let size = lcg_range(&mut seed, k.size.0, k.size.1)
                * if forward_only(k) { PLANET_SIZE_SCALE } else { 1.0 };
            // A planet waits until the last one is well clear (see
            // `PLANET_SPACING`), and until the height it must spawn at (see
            // `PLANET_MAX_SPAWN_Y`) is near enough to the view to be seen; the
            // next tick rolls again.
            let limit_y = { let (view_y, view_zoom) = frame_view_y(c); (PLANET_MAX_SPAWN_Y - view_y) * view_zoom };
            let crowded = forward_only(k) && (limit_y < -size * 0.3 || s.decor.iter().any(|d| {
                d.active && forward_only(&DECOR_KINDS[d.kind]) && d.x > VW + size * 0.6 - PLANET_SPACING
            }));
            if crowded {
                s.seed = seed;
            } else {
                let x = if k.drift.0 > 0.5 { -size * 0.6 } else { VW + size * 0.6 };
                // A few tries for a height that does not overlap a live piece.
                let mut y = lcg_range(&mut seed, DECOR_Y.0, DECOR_Y.1);
                for _ in 0..6 {
                    let clash = s.decor.iter().any(|d| {
                        d.active && (d.y - y).abs() < (d.size + size) * 0.5 + 120.0 && d.x > VW * 0.55
                    });
                    if !clash { break; }
                    y = lcg_range(&mut seed, DECOR_Y.0, DECOR_Y.1);
                }
                // Planets never start below `PLANET_MAX_SPAWN_Y` in the world:
                // pull the height up to it (it is on screen, or just above).
                if forward_only(k) && y > limit_y {
                    y = limit_y - lcg(&mut seed) * (limit_y + size * 0.3).clamp(0.0, size);
                }
                // Chosen on screen, then pinned to that spot in the sky.
                let (sky_u, sky_v) = sky.map_or((x, y), |f| f.to_sky(x, y));
                // Planets: a depth (see `depth_look`), sizing them to it.
            let (depth, size) = if forward_only(k) { roll_depth(&mut seed, size) } else { (0.0, size) };
            s.decor[slot] = DecorSlot { active: true, kind, x, y, sky_u, sky_v, depth, size, rot: k.tilt, phase: lcg(&mut seed) * 6.283, seen: false };
                s.decor_last_kind = Some(kind);
                let gap = lcg_range(&mut seed, DECOR_GAP.0, DECOR_GAP.1);
                s.decor_next_at = s.decor_travel + gap;
                s.seed = seed;
            }
        }
    }
    if s.decor_next_at <= 0.0 {
        s.decor_next_at = DECOR_FIRST_AT;
    }

    // Move: parallax with the camera, plus each kind's own drift.
    let mut rare_seen: Option<&'static str> = None;
    let mut draw: Vec<(usize, DecorSlot, f32, f32)> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    for (i, d) in s.decor.iter_mut().enumerate() {
        if !d.active { continue; }
        let k = &DECOR_KINDS[d.kind];
        let mut scale = 1.0;
        let mut sky_s = 1.0;
        if let (true, Some(f)) = (forward_only(k), sky) {
            // Forward-only across, and otherwise fixed in the sky: the sky's
            // zoom-out moves and shrinks it with everything else up there.
            let (speed, zoom_share, _) = depth_look(d.depth);
            d.sky_u -= f.sky_px(player_forward * PLANET_PARALLAX * speed / PLANET_DISTANCE_SCALE);
            (d.x, d.y) = f.to_screen(d.sky_u, d.sky_v);
            // Far planets take less of the sky's zoom-out than the sky does.
            scale = 1.0 - (1.0 - f.s) * zoom_share;
            sky_s = f.s;
        } else {
            // Self-propelled (see `self_propelled`): its own drift only.
            d.x += k.drift.0;
            d.y += k.drift.1;
        }
        d.rot += k.spin;
        let size = d.size * scale;
        let half = size * 0.6;
        let on_screen = d.x > half && d.x < VW - half;
        if on_screen && k.rare && !d.seen {
            d.seen = true;
            rare_seen = Some(k.name);
        }
        // Off either side, or drifted far out of the sky band: done.
        // Sky planets can sit above the screen until the sky zooms out, so
        // only leaving sideways retires them.
        let off_vertically = !(forward_only(k) && sky.is_some()) && (d.y < -size || d.y > VH + size);
        if d.x < -size || d.x > VW + size * 1.2 || off_vertically {
            d.active = false;
            gone.push(i);
            continue;
        }
        draw.push((i, *d, size, sky_s));
    }
    drop(s);

    for i in gone {
        for id in [decor_id(i), decor_back_id(i)] {
            if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
        }
    }
    for (i, d, size, sky_s) in draw {
        let k = &DECOR_KINDS[d.kind];
        let bob = if k.bob { (t * 0.03 + d.phase).sin() * size * 0.04 } else { 0.0 };
        // Quantised so the upscale cache holds a handful of sizes, not one
        // per spawn.
        let px = ((d.size / 32.0).round() * 32.0).max(32.0);
        let img = if forward_only(k) && d.depth >= 0.0 {
            far_planet_image(k.png, px, k.name != "spiral galaxy")
        } else {
            // Oddities, moons and junk; and a near pass, crisp and unhazed.
            crate::scenes::game::helpers::pl_image_cached(k.png, px)
        };
        let Some(img) = img else { continue };
        // Far planets behind the moving stars, the rest in front: draw with
        // one twin, hide the other.
        let behind = forward_only(k) && d.depth >= PLANET_BEHIND_STARS_DEPTH;
        let (id, other) = if behind { (decor_back_id(i), decor_id(i)) } else { (decor_id(i), decor_back_id(i)) };
        if let Some(o) = c.get_game_object_mut(&other) { o.visible = false; }
        if let Some(o) = c.get_game_object_mut(&id) {
            o.size = (size, size);
            o.position = (d.x - size * 0.5, d.y - size * 0.5 + bob);
            o.rotation = d.rot;
            o.visible = true;
            // Planets and the galaxy are solid bodies: their art is fully
            // opaque, and a tint alpha below 255 let both star layers show
            // through them. They keep their distance colouring, not its fade.
            // Planets carry their distance in their (hazed) image instead,
            // dimmed further for their depth and height (a multiply only
            // darkens, and alpha stays 255 so they stay solid).
            let (r, g, b, a) = if forward_only(k) {
                let v = (255.0 * depth_look(d.depth).2 * height_brightness(d.y, sky_s)).clamp(0.0, 255.0) as u8;
                (v, v, v, 255)
            } else { k.tint };
            o.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (size, size), 0.0),
                image: img,
                color: Some(Color(r, g, b, a)),
            });
        }
    }
    if let Some(name) = rare_seen {
        // For achievements ("I saw a whale!"): the oddity just came into view.
        c.set_var("decor_rare_seen", Value::Str(name.to_string()));
        crate::achievements::on_rare_seen(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_of_decor_decodes() {
        for k in DECOR_KINDS {
            assert!(image::load_from_memory(k.png).is_ok(), "{} does not decode", k.name);
            assert!(k.size.0 <= k.size.1 && k.parallax > 0.0 && k.weight > 0, "{}", k.name);
        }
    }

    /// A far planet stays solid where its art is solid (only its edge
    /// softens), loses contrast and is no brighter than before. With
    /// `PLANET_HAZE_DUMP=<file.png>` it also writes a sheet over the upper
    /// sky: each planet with its old flat tint, then as a far body.
    #[test]
    fn far_planets_fade_soften_and_stay_solid() {
        let sky = image::Rgba([10u8, 10, 24, 255]);
        let (cell, pad) = (260u32, 20u32);
        let planets: Vec<&DecorKind> = DECOR_KINDS.iter().filter(|k| forward_only(k)).collect();
        let mut sheet = image::RgbaImage::from_pixel(2 * (cell + pad), planets.len() as u32 * (cell + pad), sky);
        for (row, k) in planets.iter().enumerate() {
            let disc = k.name != "spiral galaxy";
            let old = crate::scenes::game::helpers::pl_image_cached(k.png, cell as f32).unwrap();
            let new = build_far_planet(k.png, cell as f32, disc).unwrap();
            let spread = |img: &image::RgbaImage| {
                let l: Vec<f32> = img.pixels().filter(|p| p[3] > 200).map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).collect();
                let m = l.iter().sum::<f32>() / l.len() as f32;
                (l.iter().map(|v| (v - m).powi(2)).sum::<f32>() / l.len() as f32).sqrt()
            };
            let solid = old.pixels().filter(|p| p[3] == 255).count();
            let lost = old.pixels().zip(new.pixels()).filter(|(a, b)| a[3] == 255 && b[3] < 200).count();
            // The galaxy is all thin arms, which softening rightly thins out.
            assert!(!disc || (lost as f32) < solid as f32 * 0.1, "{}: {lost} of {solid} solid pixels went see-through", k.name);
            assert!(spread(&new) < spread(&old), "{}: contrast not reduced", k.name);
            // Not brighter than its old flat tint (brighter reads as nearer),
            // beyond what the haze itself adds: a very dark planet is lifted
            // toward the haze a little, as real distance does.
            let mean_lum = |img: &image::RgbaImage, t: [f32; 3]| {
                let l: Vec<f32> = img.pixels().filter(|p| p[3] > 200).map(|p| 0.299 * p[0] as f32 * t[0] + 0.587 * p[1] as f32 * t[1] + 0.114 * p[2] as f32 * t[2]).collect();
                l.iter().sum::<f32>() / l.len() as f32
            };
            let tint = [k.tint.0, k.tint.1, k.tint.2].map(|t| t as f32 / 255.0);
            let haze_lum = PLANET_HAZE * (0.299 * PLANET_HAZE_RGB[0] + 0.587 * PLANET_HAZE_RGB[1] + 0.114 * PLANET_HAZE_RGB[2]);
            assert!(mean_lum(&new, [1.0; 3]) <= mean_lum(&old, tint) * 1.05 + haze_lum, "{}: brighter than before ({} vs {})", k.name, mean_lum(&new, [1.0; 3]), mean_lum(&old, tint));
            for (col, img) in [(0u32, &*old), (1, &*new)] {
                for (x, y, p) in img.enumerate_pixels() {
                    let (x, y) = (col * (cell + pad) + x, row as u32 * (cell + pad) + y);
                    if x >= sheet.width() || y >= sheet.height() { continue; }
                    let a = p[3] as f32 / 255.0;
                    let tint = if col == 0 { [k.tint.0, k.tint.1, k.tint.2].map(|t| t as f32 / 255.0) } else { [1.0; 3] };
                    let b = *sheet.get_pixel(x, y);
                    let px = [0, 1, 2].map(|c| (p[c] as f32 * tint[c] * a + b[c] as f32 * (1.0 - a)) as u8);
                    sheet.put_pixel(x, y, image::Rgba([px[0], px[1], px[2], 255]));
                }
            }
        }
        if let Ok(path) = std::env::var("PLANET_HAZE_DUMP") { sheet.save(path).unwrap(); }
    }

    /// A renamed kind would silently fall out of the list and start moving
    /// backwards again.
    #[test]
    fn forward_only_is_exactly_the_planets_and_the_galaxy() {
        for name in FORWARD_ONLY {
            let k = DECOR_KINDS.iter().find(|k| k.name == *name)
                .unwrap_or_else(|| panic!("{name} is not a decor kind"));
            assert_eq!(k.drift, (0.0, 0.0), "{name} moves on its own");
        }
        // The planets, the galaxy, and the (far) cratered moon: only far
        // bodies are pinned in the sky; everything else floats on its own.
        assert_eq!(DECOR_KINDS.iter().filter(|k| forward_only(k)).count(), 8);
        assert!(DECOR_KINDS.iter().all(|k| k.name != "tiny moon"), "the close moon is gone");
        for k in DECOR_KINDS.iter().filter(|k| !forward_only(k)) {
            assert!(self_propelled(k), "{} would sway with the camera", k.name);
        }
    }

    /// A planet must land where a grab node at the same world height is
    /// drawn. Decor runs before the engine's camera stage, so the prediction
    /// is checked against that stage itself, not against a re-derivation.
    #[test]
    fn frame_view_y_matches_the_engine_camera_this_frame() {
        let (mut ctx, _requests) = Context::new();
        let mut c = Canvas::new(&mut ctx, CanvasMode::Landscape);
        c.add_game_object("player".into(), GameObject::build("player").size(80.0, 80.0).position(5_000.0, -900.0).finish());
        let world_y = 400.0_f32;
        // Mid zoom-out, anchored on the player at ground level (tick_zoom),
        // then a camera with no anchor following the player.
        for anchor in [Some((5_040.0, VH)), None] {
            let mut cam = Camera::new((1_000_000_000.0, 1_000_000.0), (VW, VH));
            cam.follow(Some(Target::name("player")));
            cam.position = (3_000.0, 1_200.0);
            cam.zoom = 0.9;
            cam.zoom_target = 0.55;
            cam.zoom_lerp_speed = 0.05;
            cam.zoom_anchor = anchor;
            c.set_camera(cam);
            let (vy, vz) = frame_view_y(&c);
            let predicted = (world_y - vy) * vz;
            c.apply_camera_transform_for_test();
            let cam = c.camera().unwrap();
            let rendered = (world_y - cam.position.1) * cam.zoom;
            assert!((predicted - rendered).abs() < 0.01,
                "anchor {anchor:?}: planet at {predicted}, a node at its height at {rendered}");
        }
    }

    #[test]
    fn oddities_are_rare_and_never_twice_in_a_row() {
        let mut seed = 12345u64;
        let mut last = None;
        let (mut rare, mut n) = (0, 0);
        for _ in 0..4000 {
            let k = pick_kind(&mut seed, last, false);
            assert_ne!(Some(k), last, "the same piece twice in a row");
            if DECOR_KINDS[k].rare { rare += 1; }
            n += 1;
            last = Some(k);
        }
        let share = rare as f32 / n as f32;
        assert!(share > 0.02 && share < 0.12, "oddities are {:.1}% of the sky", share * 100.0);
        // Never a second oddity while one is up.
        for _ in 0..500 {
            let k = pick_kind(&mut seed, None, true);
            assert!(!DECOR_KINDS[k].rare);
        }
    }
}
