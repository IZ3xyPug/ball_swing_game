//! cosmetics.rs — what the shop sells beyond a colour.
//!
//! Two kinds of look, each bought once and kept for good:
//!
//!   ROPE STYLES — a tile drawn along the rope by `Effect::TiledStrip`, so a
//!     style looks right at any length (the classic energy rope is one GIF
//!     stretched end to end: squashed when short, smeared when long). Every
//!     style takes every rope COLOUR — the tiles are neutral light and are
//!     gradient-mapped to the colour — so style and colour are picked
//!     separately in the shop. Colour 0, ORIGINAL, wears the style's own
//!     signature colour (the art itself is white, which read as unfinished).
//!   CAT BREEDS — animated cat balls in the calico's style: curled up, half
//!     uncurled, spread-eagle. The game's own frame logic drives them.
//!
//! Prices are in META today. `Price::Premium` is reserved for the premium
//! currency (`PlayerProfile::premium_currency`): moving an item behind real
//! money is a one-word change in these tables.
//!
//! Art: assets/pixellab/ropes and assets/pixellab/cats (provenance in
//! SOURCES.md).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Price {
    Free,
    Meta(u64),
    /// For the premium currency, when it exists. Cannot be bought yet.
    Premium(u64),
}

impl Price {
    pub fn label(self) -> String {
        match self {
            Price::Free => "FREE".to_string(),
            Price::Meta(n) => format!("{n} META"),
            Price::Premium(n) => format!("{n} GEMS"),
        }
    }
}

// ── Rope styles ──────────────────────────────────────────────────────────────

pub struct RopeStyle {
    pub id: &'static str,
    pub name: &'static str,
    pub price: Price,
    /// Tile frames (64 px PNGs). Empty for the classic energy rope, which
    /// keeps its own GIF path.
    pub frames: &'static [&'static [u8]],
    /// Animation frames per second, and whether it plays back and forth.
    pub fps: f32,
    pub pingpong: bool,
    /// Cells per second the pattern runs from the hook toward the ball
    /// (a cell is one repeat: `cell_len`).
    pub flow: f32,
    /// Flip every other cell. Joins any tile without a gap, but a pattern
    /// reads as reflected at each join, so a periodic tile is cropped to
    /// whole repeats (`crop`) and tiled straight instead.
    pub mirror: bool,
    /// World px across the rope's quad, and world px a whole 64 px tile
    /// covers along it.
    pub beam: f32,
    pub tile_len: f32,
    /// Rows of the tile to keep, (first, count): a whole number of the
    /// pattern's own repeats, so cell after cell joins with no seam. (0, 0)
    /// keeps the whole tile. Found by matching rows a repeat apart; the
    /// `a_cropped_tile_joins_itself` test pins it to the art.
    pub crop: (u32, u32),
    /// The style's own colour, worn when the player picks ORIGINAL.
    pub signature: (u8, u8, u8),
}

macro_rules! tile {
    ($n:literal) => {
        include_bytes!(concat!("../assets/pixellab/ropes/", $n, ".png"))
    };
}

/// A style's nine-frame "flow" animation.
macro_rules! flow9 {
    ($n:literal) => {
        [
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/0.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/1.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/2.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/3.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/4.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/5.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/6.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/7.png")),
            include_bytes!(concat!("../assets/pixellab/ropes/", $n, "_flow/8.png")),
        ]
    };
}

/// These four play ONE frame, moved by `flow`. Their PixelLab "flow"
/// animations change the pattern's shape from frame to frame — sparkle
/// bursts and a flattening on the helix, three different link designs on
/// the chain — which read as the rope breaking. A still scrolled along the
/// rope moves smoothly (a scrolling helix reads as a spinning one).
const HELIX: [&[u8]; 1] = [include_bytes!("../assets/pixellab/ropes/helix_flow/0.png")];
const CHAIN: [&[u8]; 1] = [include_bytes!("../assets/pixellab/ropes/chain_flow/8.png")];
const LIGHTNING: [&[u8]; 9] = [
    include_bytes!("../assets/pixellab/ropes/lightning_flow/0.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/1.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/2.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/3.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/4.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/5.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/6.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/7.png"),
    include_bytes!("../assets/pixellab/ropes/lightning_flow/8.png"),
];
const VOID: [&[u8]; 1] = [include_bytes!("../assets/pixellab/ropes/void_flow/2.png")];
const BRAID: [&[u8]; 1] = [tile!("braid")];
const STARDUST: [&[u8]; 9] = flow9!("stardust");
const FLAME: [&[u8]; 9] = flow9!("flame");
const CIRCUIT: [&[u8]; 1] = [tile!("circuit")];
const SPECTRAL: [&[u8]; 1] = [include_bytes!("../assets/pixellab/ropes/spectral_flow/0.png")];
const VINE: [&[u8]; 1] = [tile!("vine")];
const RUNIC: [&[u8]; 1] = [tile!("runic")];
const RIBBON: [&[u8]; 1] = [tile!("ribbon")];

/// Index 0 is the classic energy rope; the order is the shop's order and is
/// SAVED (`cosmetic_rope_style`), so only ever append.
pub const ROPE_STYLES: &[RopeStyle] = &[
    RopeStyle { id: "energy", name: "ENERGY", price: Price::Free, frames: &[], fps: 0.0, pingpong: false, flow: 0.0, mirror: false, beam: 60.0, tile_len: 60.0, crop: (0, 0), signature: (90, 170, 240) },
    RopeStyle { id: "helix", name: "HELIX", price: Price::Meta(400), frames: &HELIX, fps: 0.0, pingpong: false, flow: 2.4, mirror: false, beam: 139.2, tile_len: 139.2, crop: (14, 16), signature: (70, 225, 255) },
    RopeStyle { id: "chain", name: "PLASMA CHAIN", price: Price::Meta(350), frames: &CHAIN, fps: 0.0, pingpong: false, flow: 0.6, mirror: false, beam: 139.2, tile_len: 139.2, crop: (28, 32), signature: (255, 80, 190) },
    RopeStyle { id: "lightning", name: "LIGHTNING", price: Price::Meta(550), frames: &LIGHTNING, fps: 14.0, pingpong: false, flow: 0.0, mirror: true, beam: 150.8, tile_len: 150.8, crop: (0, 0), signature: (255, 240, 120) },
    RopeStyle { id: "braid", name: "LIGHT BRAID", price: Price::Meta(300), frames: &BRAID, fps: 0.0, pingpong: false, flow: 0.45, mirror: true, beam: 139.2, tile_len: 139.2, crop: (0, 0), signature: (255, 195, 90) },
    RopeStyle { id: "stardust", name: "STARDUST", price: Price::Meta(450), frames: &STARDUST, fps: 8.0, pingpong: false, flow: 0.8, mirror: true, beam: 139.2, tile_len: 139.2, crop: (0, 0), signature: (205, 165, 255) },
    RopeStyle { id: "flame", name: "SOLAR FLAME", price: Price::Meta(600), frames: &FLAME, fps: 10.0, pingpong: false, flow: 1.1, mirror: false, beam: 150.8, tile_len: 150.8, crop: (0, 0), signature: (255, 110, 30) },
    RopeStyle { id: "void", name: "VOID THREAD", price: Price::Meta(750), frames: &VOID, fps: 0.0, pingpong: false, flow: 0.9, mirror: false, beam: 139.2, tile_len: 139.2, crop: (20, 26), signature: (150, 60, 255) },
    RopeStyle { id: "circuit", name: "CIRCUIT", price: Price::Meta(450), frames: &CIRCUIT, fps: 0.0, pingpong: false, flow: 1.3, mirror: true, beam: 139.2, tile_len: 139.2, crop: (0, 0), signature: (50, 255, 130) },
    RopeStyle { id: "spectral", name: "SPECTRAL CHAIN", price: Price::Meta(550), frames: &SPECTRAL, fps: 0.0, pingpong: false, flow: 0.3, mirror: true, beam: 145.0, tile_len: 145.0, crop: (0, 0), signature: (130, 255, 225) },
    RopeStyle { id: "vine", name: "LUMEN VINE", price: Price::Meta(400), frames: &VINE, fps: 0.0, pingpong: false, flow: 0.2, mirror: true, beam: 145.0, tile_len: 145.0, crop: (0, 0), signature: (160, 255, 70) },
    RopeStyle { id: "runic", name: "RUNIC", price: Price::Meta(650), frames: &RUNIC, fps: 0.0, pingpong: false, flow: 0.5, mirror: true, beam: 139.2, tile_len: 139.2, crop: (0, 0), signature: (110, 140, 255) },
    RopeStyle { id: "ribbon", name: "RIBBON", price: Price::Meta(350), frames: &RIBBON, fps: 0.0, pingpong: false, flow: 0.9, mirror: true, beam: 139.2, tile_len: 139.2, crop: (0, 0), signature: (255, 105, 120) },
];

/// The shop carousel's names and card colours for the ROPES category, in
/// `ROPE_STYLES` order (a test pins the lengths together).
pub const ROPE_STYLE_NAMES: &[&str] = &[
    "ENERGY", "HELIX", "PLASMA CHAIN", "LIGHTNING", "LIGHT BRAID", "STARDUST", "SOLAR FLAME",
    "VOID THREAD", "CIRCUIT", "SPECTRAL CHAIN", "LUMEN VINE", "RUNIC", "RIBBON",
];
/// Each style's signature colour (a test pins these to `RopeStyle::signature`).
pub const ROPE_STYLE_SWATCHES: &[(u8, u8, u8)] = &[
    (90, 170, 240), (70, 225, 255), (255, 80, 190), (255, 240, 120), (255, 195, 90), (205, 165, 255),
    (255, 110, 30), (150, 60, 255), (50, 255, 130), (130, 255, 225), (160, 255, 70), (110, 140, 255), (255, 105, 120),
];

pub fn rope_key(style: usize) -> String {
    format!("rope:{}", ROPE_STYLES.get(style).map(|s| s.id).unwrap_or("energy"))
}

/// Gradient-map `src` to `rgb`: dark texels to a deep shade of it, the
/// middle to the colour itself, the brightest to white-hot — so neutral
/// white art takes any colour and keeps its hot core.
pub fn gradient_map(src: &image::RgbaImage, rgb: (u8, u8, u8)) -> image::RgbaImage {
    let (cr, cg, cb) = (rgb.0 as f32, rgb.1 as f32, rgb.2 as f32);
    let mut out = src.clone();
    for p in out.pixels_mut() {
        if p[3] == 0 {
            continue;
        }
        let l = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
        const MID: f32 = 0.62;
        const LOW: f32 = 0.22;
        let (r, g, b) = if l < MID {
            let t = l / MID;
            (cr * (LOW + (1.0 - LOW) * t), cg * (LOW + (1.0 - LOW) * t), cb * (LOW + (1.0 - LOW) * t))
        } else {
            let t = (l - MID) / (1.0 - MID);
            (cr + (255.0 - cr) * t, cg + (255.0 - cg) * t, cb + (255.0 - cb) * t)
        };
        p[0] = r.round().clamp(0.0, 255.0) as u8;
        p[1] = g.round().clamp(0.0, 255.0) as u8;
        p[2] = b.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Mean luminance a style's tile is brought down to before its gradient map.
const EXPOSE_MEAN: f32 = 0.72;

/// A style's tile in colour `rgb`: `expose`d, then gradient-mapped.
pub fn tint_tile(src: &image::RgbaImage, rgb: (u8, u8, u8)) -> image::RgbaImage {
    gradient_map(&expose(src), rgb)
}

/// Bring a bright tile's mean luminance down to `EXPOSE_MEAN`, so it takes
/// its colour instead of mapping almost wholly to the white-hot end (circuit,
/// runic, flame and vine came out near white). A gamma keeps the brightest
/// texels white, the hot core; a tile of flat white has no core to keep and
/// is scaled down instead. Dark tiles (the void thread) are left alone.
pub fn expose(src: &image::RgbaImage) -> image::RgbaImage {
    let (gamma, scale) = exposure_params(src);
    apply_exposure(src, gamma, scale)
}

fn luminance(p: &image::Rgba<u8>) -> f32 {
    (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0
}

fn mean_luminance(img: &image::RgbaImage) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0u32);
    for p in img.pixels().filter(|p| p[3] > 128) {
        sum += luminance(p);
        n += 1;
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}

/// The (gamma, scale) that `expose` applies to `src`: (1, 1) for a tile
/// already at or under `EXPOSE_MEAN`.
fn exposure_params(src: &image::RgbaImage) -> (f32, f32) {
    let mean = mean_luminance(src);
    if mean <= EXPOSE_MEAN {
        return (1.0, 1.0);
    }
    let gamma = EXPOSE_MEAN.ln() / mean.min(0.999).ln();
    let after = mean_luminance(&apply_exposure(src, gamma, 1.0));
    let scale = if after > EXPOSE_MEAN + 0.05 { EXPOSE_MEAN / after } else { 1.0 };
    (gamma, scale)
}

/// Luminance `l` becomes `l^gamma * scale`, hue kept.
fn apply_exposure(src: &image::RgbaImage, gamma: f32, scale: f32) -> image::RgbaImage {
    let mut out = src.clone();
    if gamma == 1.0 && scale == 1.0 {
        return out;
    }
    for p in out.pixels_mut() {
        let l = luminance(p);
        if p[3] == 0 || l <= 0.0 {
            continue;
        }
        let k = l.powf(gamma) * scale / l;
        for ch in 0..3 {
            p[ch] = (p[ch] as f32 * k).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// The colour a style is drawn in: rope colour `color_idx` (its `rgb`), or
/// the style's signature for colour 0, ORIGINAL.
pub fn rope_tint(style: usize, color_idx: usize, rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    match ROPE_STYLES.get(style) {
        Some(s) if color_idx == 0 => s.signature,
        _ => rgb,
    }
}

/// The source tiles' edge, in px.
const TILE_PX: f32 = 64.0;

impl RopeStyle {
    /// World px one repeat of the pattern covers along the rope: the whole
    /// tile, or the cropped rows at the same pixel scale.
    pub fn cell_len(&self) -> f32 {
        if self.crop.1 == 0 { self.tile_len } else { self.tile_len * self.crop.1 as f32 / TILE_PX }
    }
}

/// The rows of `img` a style keeps (see `RopeStyle::crop`).
fn crop_rows(img: image::RgbaImage, crop: (u32, u32)) -> image::RgbaImage {
    if crop.1 == 0 || crop.0 + crop.1 > img.height() {
        return img;
    }
    image::imageops::crop_imm(&img, 0, crop.0, img.width(), crop.1).to_image()
}

/// Frame `frame` of a style's art, cropped and exposed but not yet
/// coloured. Exposure is measured on frame 0 and applied to every frame,
/// so an animation does not flicker in brightness from frame to frame.
fn style_frame_art(style: &RopeStyle, frame: usize) -> Option<image::RgbaImage> {
    let load = |i: usize| -> Option<image::RgbaImage> {
        Some(crop_rows(image::load_from_memory(style.frames.get(i)?).ok()?.to_rgba8(), style.crop))
    };
    let (gamma, scale) = exposure_params(&load(0)?);
    Some(apply_exposure(&load(frame)?, gamma, scale))
}

/// A style's card art in colour `rgb`: its first frame repeated the way the
/// rope draws it, about two tiles long.
pub fn rope_card_art(style: usize, rgb: (u8, u8, u8)) -> Option<image::RgbaImage> {
    let s = ROPE_STYLES.get(style)?;
    let cell = gradient_map(&style_frame_art(s, 0)?, rgb);
    let (w, h) = cell.dimensions();
    let n = ((2.0 * TILE_PX) / h as f32).round().max(1.0) as u32;
    let mut strip = image::RgbaImage::new(w, h * n);
    for k in 0..n {
        let c = if s.mirror && k % 2 == 1 { image::imageops::flip_vertical(&cell) } else { cell.clone() };
        image::imageops::overlay(&mut strip, &c, 0, (k * h) as i64);
    }
    Some(strip)
}

/// Which animation frame a style shows at `t` seconds.
pub fn rope_frame(style: &RopeStyle, t: f32) -> usize {
    let n = style.frames.len().max(1);
    if n == 1 || style.fps <= 0.0 {
        return 0;
    }
    let k = (t * style.fps) as usize;
    if style.pingpong {
        let period = 2 * n - 2;
        let i = k % period;
        if i < n { i } else { period - i }
    } else {
        k % n
    }
}

/// One tile of a style, frame `frame`, gradient-mapped to `rgb` (see
/// `rope_tint`), upscaled 4x with nearest so linear sampling in the shader
/// keeps the pixel art crisp. Cached.
pub fn rope_tile(style: usize, frame: usize, rgb: (u8, u8, u8)) -> Option<Arc<image::RgbaImage>> {
    type Key = (usize, usize, (u8, u8, u8));
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<image::RgbaImage>>>> = OnceLock::new();
    let s = ROPE_STYLES.get(style)?;
    let bytes = s.frames.get(frame)?;
    let key = (style, frame, rgb);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(img) = cache.lock().unwrap().get(&key) {
        return Some(img.clone());
    }
    let _ = bytes;
    let coloured = gradient_map(&style_frame_art(s, frame)?, rgb);
    let (w, h) = coloured.dimensions();
    let big = image::imageops::resize(&coloured, w * 4, h * 4, image::imageops::FilterType::Nearest);
    let out = Arc::new(big);
    let mut g = cache.lock().unwrap();
    if g.len() > 400 {
        g.clear();
    }
    g.insert(key, out.clone());
    Some(out)
}

// ── Cat breeds ───────────────────────────────────────────────────────────────

pub struct CatSkin {
    pub id: &'static str,
    pub name: &'static str,
    pub price: Price,
    pub swatch: (u8, u8, u8),
    /// Curled ball, half uncurled, spread-eagle: the game plays toward the
    /// last frame while the ball rises and back to the first when it hooks.
    pub frames: [&'static [u8]; 3],
}

/// Characters before the breeds: the calico and six colours
/// (`PLAYER_CHAR_COLORS[..FIRST_SKIN_CHAR]`). Breeds follow, in this order,
/// which is SAVED — only ever append.
pub const FIRST_SKIN_CHAR: usize = 7;

pub const CAT_SKINS: &[CatSkin] = &[
    CatSkin { id: "ginger", name: "GINGER", price: Price::Meta(150), swatch: (230, 140, 60),
              frames: [include_bytes!("../assets/pixellab/cats/ginger_ball.png"),
                       include_bytes!("../assets/pixellab/cats/ginger_half.png"),
                       include_bytes!("../assets/pixellab/cats/ginger_open.png")] },
    CatSkin { id: "black", name: "MIDNIGHT", price: Price::Meta(150), swatch: (50, 45, 60),
              frames: [include_bytes!("../assets/pixellab/cats/black_ball.png"),
                       include_bytes!("../assets/pixellab/cats/black_half.png"),
                       include_bytes!("../assets/pixellab/cats/black_open.png")] },
    CatSkin { id: "siamese", name: "SIAMESE", price: Price::Meta(150), swatch: (225, 205, 175),
              frames: [include_bytes!("../assets/pixellab/cats/siamese_ball.png"),
                       include_bytes!("../assets/pixellab/cats/siamese_half.png"),
                       include_bytes!("../assets/pixellab/cats/siamese_open.png")] },
    CatSkin { id: "tuxedo", name: "TUXEDO", price: Price::Meta(150), swatch: (30, 30, 35),
              frames: [include_bytes!("../assets/pixellab/cats/tuxedo_ball.png"),
                       include_bytes!("../assets/pixellab/cats/tuxedo_half.png"),
                       include_bytes!("../assets/pixellab/cats/tuxedo_open.png")] },
    CatSkin { id: "silver", name: "SILVER TABBY", price: Price::Meta(150), swatch: (170, 170, 175),
              frames: [include_bytes!("../assets/pixellab/cats/silver_ball.png"),
                       include_bytes!("../assets/pixellab/cats/silver_half.png"),
                       include_bytes!("../assets/pixellab/cats/silver_open.png")] },
    CatSkin { id: "mainecoon", name: "MAINE COON", price: Price::Meta(250), swatch: (140, 100, 60),
              frames: [include_bytes!("../assets/pixellab/cats/mainecoon_ball.png"),
                       include_bytes!("../assets/pixellab/cats/mainecoon_half.png"),
                       include_bytes!("../assets/pixellab/cats/mainecoon_open.png")] },
    CatSkin { id: "persian", name: "PERSIAN", price: Price::Meta(250), swatch: (240, 240, 245),
              frames: [include_bytes!("../assets/pixellab/cats/persian_ball.png"),
                       include_bytes!("../assets/pixellab/cats/persian_half.png"),
                       include_bytes!("../assets/pixellab/cats/persian_open.png")] },
    CatSkin { id: "sphynx", name: "SPHYNX", price: Price::Meta(250), swatch: (230, 170, 160),
              frames: [include_bytes!("../assets/pixellab/cats/sphynx_ball.png"),
                       include_bytes!("../assets/pixellab/cats/sphynx_half.png"),
                       include_bytes!("../assets/pixellab/cats/sphynx_open.png")] },
    CatSkin { id: "galaxy", name: "GALAXY", price: Price::Meta(700), swatch: (110, 80, 200),
              frames: [include_bytes!("../assets/pixellab/cats/galaxy_ball.png"),
                       include_bytes!("../assets/pixellab/cats/galaxy_half.png"),
                       include_bytes!("../assets/pixellab/cats/galaxy_open.png")] },
    CatSkin { id: "robot", name: "ROBO-CAT", price: Price::Meta(700), swatch: (190, 205, 215),
              frames: [include_bytes!("../assets/pixellab/cats/robot_ball.png"),
                       include_bytes!("../assets/pixellab/cats/robot_half.png"),
                       include_bytes!("../assets/pixellab/cats/robot_open.png")] },
    CatSkin { id: "astronaut", name: "ASTRO-CAT", price: Price::Meta(800), swatch: (240, 240, 250),
              frames: [include_bytes!("../assets/pixellab/cats/astronaut_ball.png"),
                       include_bytes!("../assets/pixellab/cats/astronaut_half.png"),
                       include_bytes!("../assets/pixellab/cats/astronaut_open.png")] },
    CatSkin { id: "magma", name: "MAGMA", price: Price::Meta(800), swatch: (240, 100, 30),
              frames: [include_bytes!("../assets/pixellab/cats/magma_ball.png"),
                       include_bytes!("../assets/pixellab/cats/magma_half.png"),
                       include_bytes!("../assets/pixellab/cats/magma_open.png")] },
];

pub fn cat_skin(char_idx: usize) -> Option<&'static CatSkin> {
    char_idx.checked_sub(FIRST_SKIN_CHAR).and_then(|i| CAT_SKINS.get(i))
}

pub fn char_price(char_idx: usize) -> Price {
    cat_skin(char_idx).map(|s| s.price).unwrap_or(Price::Free)
}

pub fn char_key(char_idx: usize) -> String {
    match cat_skin(char_idx) {
        Some(s) => format!("cat:{}", s.id),
        None => format!("char:{char_idx}"),
    }
}

/// The breed's frames decoded at `size`, for an `AnimatedSprite`.
pub fn cat_sprite(char_idx: usize, size: f32, fps: f32) -> Option<quartz::AnimatedSprite> {
    let skin = cat_skin(char_idx)?;
    crate::scenes::game::helpers::pl_sprite(&skin.frames, size, fps)
}

// ── Ownership ────────────────────────────────────────────────────────────────

/// Owned: free, or bought on this profile.
pub fn owns(key: &str, price: Price) -> bool {
    price == Price::Free || crate::profile::owns_cosmetic(key)
}

/// Buy `key` for `price`. `Err(shortfall)` when META is short; premium items
/// cannot be bought until the premium currency exists (`Err(u64::MAX)`).
pub fn buy(key: &str, price: Price) -> Result<(), u64> {
    match price {
        Price::Free => Ok(()),
        Price::Premium(_) => Err(u64::MAX),
        Price::Meta(cost) => crate::profile::buy_cosmetic(key, cost),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shop_lists_match_the_rope_table() {
        assert_eq!(ROPE_STYLE_NAMES.len(), ROPE_STYLES.len());
        assert_eq!(ROPE_STYLE_SWATCHES.len(), ROPE_STYLES.len());
        for ((s, n), sw) in ROPE_STYLES.iter().zip(ROPE_STYLE_NAMES).zip(ROPE_STYLE_SWATCHES) {
            assert_eq!(s.name, *n);
            assert_eq!(s.signature, *sw, "{}: the card swatch is not its signature colour", s.id);
        }
    }

    #[test]
    fn a_bright_tile_takes_its_colour_and_keeps_its_core() {
        let lit = |img: &image::RgbaImage, x: u32| *img.get_pixel(x, 0);
        // Mostly bright grey with one white-hot texel.
        let mut tile = image::RgbaImage::from_pixel(8, 1, image::Rgba([235, 235, 235, 255]));
        tile.put_pixel(7, 0, image::Rgba([255, 255, 255, 255]));
        let out = tint_tile(&tile, (255, 110, 30));
        let body = lit(&out, 0);
        assert!(body[0] > 200 && body[2] < 150, "the body should read orange, got {body:?}");
        assert_eq!(lit(&out, 7), image::Rgba([255, 255, 255, 255]), "the hot core stays white");
        // Flat white has no core: it is scaled down to the colour, not left white.
        let flat = image::RgbaImage::from_pixel(4, 1, image::Rgba([255, 255, 255, 255]));
        let f = lit(&tint_tile(&flat, (110, 140, 255)), 0);
        assert!(f[0] < 200, "flat white should take the colour, got {f:?}");
        // A dark tile is left as it is.
        let dark = image::RgbaImage::from_pixel(4, 1, image::Rgba([80, 80, 80, 255]));
        assert_eq!(expose(&dark), dark);
    }

    #[test]
    fn a_cropped_tile_joins_itself() {
        // The row after a crop must repeat its first row, or every join
        // along the rope shows. Compared as opaque masks.
        for s in ROPE_STYLES.iter().filter(|s| s.crop.1 > 0) {
            let img = image::load_from_memory(s.frames[0]).unwrap().to_rgba8();
            let (first, n) = s.crop;
            assert!(first + n < img.height(), "{}: crop runs off the tile", s.id);
            assert!(!s.mirror, "{}: a cropped tile tiles straight", s.id);
            let row = |y: u32| (0..img.width()).map(|x| img.get_pixel(x, y)[3] > 40).collect::<Vec<_>>();
            let (a, b) = (row(first), row(first + n));
            let differ = a.iter().zip(&b).filter(|(p, q)| p != q).count();
            assert!(differ <= 1, "{}: the crop's seam differs in {differ} px", s.id);
        }
    }

    #[test]
    fn original_wears_the_signature_and_other_colours_win() {
        assert_eq!(rope_tint(6, 0, (1, 2, 3)), ROPE_STYLES[6].signature);
        assert_eq!(rope_tint(6, 4, (1, 2, 3)), (1, 2, 3));
    }

    #[test]
    fn every_tile_and_cat_frame_decodes() {
        for s in ROPE_STYLES {
            for f in s.frames {
                assert!(image::load_from_memory(f).is_ok(), "{} tile does not decode", s.id);
            }
        }
        for c in CAT_SKINS {
            for f in c.frames {
                let img = image::load_from_memory(f).expect(c.id).to_rgba8();
                assert_eq!(img.dimensions(), (128, 128), "{}", c.id);
            }
        }
        assert_eq!(
            crate::constants::PLAYER_CHAR_COLORS.len(),
            FIRST_SKIN_CHAR + CAT_SKINS.len(),
            "every breed needs a roster entry"
        );
    }

    #[test]
    fn a_gradient_map_keeps_the_hot_core_and_takes_the_colour() {
        let src = image::RgbaImage::from_fn(3, 1, |x, _| match x {
            0 => image::Rgba([40, 40, 40, 255]),
            1 => image::Rgba([158, 158, 158, 255]),
            _ => image::Rgba([255, 255, 255, 255]),
        });
        let out = gradient_map(&src, (255, 40, 40));
        assert!(out.get_pixel(0, 0)[0] > out.get_pixel(0, 0)[1] * 2, "dark end takes the hue");
        assert!(out.get_pixel(1, 0)[0] > 240 && out.get_pixel(1, 0)[1] < 90, "middle is the colour");
        assert_eq!(out.get_pixel(2, 0).0, [255, 255, 255, 255], "white stays white-hot");
    }

    #[test]
    fn pingpong_frames_run_there_and_back() {
        // No shipped style plays back and forth now (the void thread is a
        // still); the mechanism stays for art whose loop does not wrap.
        const SIX: [&[u8]; 6] = [&[]; 6];
        let s = &RopeStyle {
            id: "t", name: "T", price: Price::Free, frames: &SIX, fps: 7.0, pingpong: true, flow: 0.0,
            mirror: true, beam: 1.0, tile_len: 1.0, crop: (0, 0), signature: (0, 0, 0),
        };
        let seq: Vec<usize> = (0..12).map(|k| rope_frame(s, k as f32 / s.fps + 1e-4)).collect();
        assert_eq!(seq, vec![0, 1, 2, 3, 4, 5, 4, 3, 2, 1, 0, 1]);
    }
}
