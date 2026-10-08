//! fx_kit.rs — the shared toolkit for the bosses' attack effects.
//!
//! Every boss dresses its attacks in its own module (`*_fx.rs`), in the way
//! `colossus_finale.rs` dresses the Colossus's last stand: it reads what the
//! fight already draws and layers art over it, never touching the fight's
//! logic. This is what those modules share:
//!
//! * a handful of white-based particle shapes (tinted per use), generated
//!   once and kept as shared `Arc`s, so nothing uploads a texture per frame;
//! * `Pool`, a fixed set of particle objects: emit, step, draw;
//! * `Bolts`, jagged lightning drawn from thin quads;
//! * `put` / `hide` for one-off quads.
//!
//! Everything here is UNLIT: an attack is light, and in a boss's darkness
//! (the Sun Devourer's whole fight) a lit effect would vanish with the scene.
//!
//! LAYERS. The fights' shader effects (beams, arcs, threads, lanes) sit on
//! layers 20–21 and are drawn after every sprite, depth-tested. A particle on
//! a higher layer writes depth across its whole quad, faint edges included,
//! and the effect behind it then fails there — a dark hole wherever a spark
//! crosses a beam. So anything that rides along an effect goes on layer 19,
//! under it, and the effect blends over it.

use quartz::*;
use std::sync::{Arc, OnceLock};

pub type Img = Arc<image::RgbaImage>;

pub fn cached(slot: &'static OnceLock<Img>, f: impl FnOnce() -> image::RgbaImage) -> Img {
    slot.get_or_init(|| Arc::new(f())).clone()
}

fn mix(stops: &[(f32, [f32; 4])], d: f32) -> [u8; 4] {
    let i = stops.iter().position(|s| s.0 >= d).unwrap_or(stops.len() - 1).max(1);
    let (a, b) = (stops[i - 1], stops[i]);
    let t = ((d - a.0) / (b.0 - a.0).max(1e-4)).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    [0, 1, 2, 3].map(|k| (a.1[k] + (b.1[k] - a.1[k]) * t).clamp(0.0, 255.0) as u8)
}

/// A radial gradient through `stops` (radius 0..1 → rgba).
pub fn radial(n: u32, stops: &[(f32, [f32; 4])]) -> image::RgbaImage {
    let r = n as f32 * 0.5;
    image::RgbaImage::from_fn(n, n, |x, y| {
        let d = (((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt() / r).min(1.0);
        image::Rgba(mix(stops, d))
    })
}

/// A strip's cross-section (stretched along its length): `stops` from the
/// centre line (0) to the edge (1).
///
/// Drawn very WIDE (64:1). An unlit image keeps its aspect by cropping
/// ("cover"), and a square band stretched down a long strip was cropped to
/// its middle row — the gradient across it vanished into a flat slab. At
/// 64:1 any strip up to that long crops along its length, which is uniform.
pub fn band(stops: &[(f32, [f32; 4])]) -> image::RgbaImage {
    let (w, h) = (2048u32, 32u32);
    image::RgbaImage::from_fn(w, h, |_, y| {
        let d = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
        image::Rgba(mix(stops, d))
    })
}

// ── Particle shapes (white; tinted per use) ──────────────────────────────────

pub const DOT: usize = 0;
pub const SPARK: usize = 1;
pub const SMOKE: usize = 2;
pub const RING: usize = 3;
pub const SHARD: usize = 4;
pub const FLAME: usize = 5;
pub const HALO: usize = 6;
pub const STAR: usize = 7;
pub const NOTE: usize = 8;
const SHAPES: usize = 9;
/// A pool's own art (`Pool::extra`): `EXTRA + i` is its `i`th image.
pub const EXTRA: usize = 100;

fn shape(i: usize) -> image::RgbaImage {
    match i {
        // A soft dot with a hot middle.
        DOT => radial(32, &[(0.0, [255.0; 4]), (0.3, [255.0, 255.0, 255.0, 220.0]), (1.0, [255.0, 255.0, 255.0, 0.0])]),
        // A streak: bright head on the right, tail fading left.
        SPARK => {
            let (w, h) = (128u32, 12u32);
            image::RgbaImage::from_fn(w, h, |x, y| {
                let fx = (x as f32 + 0.5) / w as f32;
                let fy = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
                let a = fx.powf(1.6) * (1.0 - fy).powf(1.2) * (1.0 - ((fx - 0.97) / 0.03).clamp(0.0, 1.0));
                image::Rgba([255, 255, 255, (a * 255.0) as u8])
            })
        }
        // A smoke puff: lumpy, soft.
        SMOKE => {
            let n = 64u32;
            image::RgbaImage::from_fn(n, n, |x, y| {
                let (fx, fy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                let ang = fy.atan2(fx);
                let lump = 0.82 + 0.1 * (ang * 3.0).sin() + 0.06 * (ang * 7.0 + 1.3).sin();
                let d = (fx * fx + fy * fy).sqrt() / lump;
                let a = (1.0 - d).clamp(0.0, 1.0).powf(0.9) * 0.85;
                image::Rgba([255, 255, 255, (a * 255.0) as u8])
            })
        }
        // A thin soft annulus.
        RING => radial(128, &[(0.0, [255.0, 255.0, 255.0, 0.0]), (0.72, [255.0, 255.0, 255.0, 0.0]),
                              (0.88, [255.0, 255.0, 255.0, 255.0]), (1.0, [255.0, 255.0, 255.0, 0.0])]),
        // A shard: a sharp diamond, lit edge.
        SHARD => {
            let n = 32u32;
            image::RgbaImage::from_fn(n, n, |x, y| {
                let (fx, fy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                let d = fx.abs() * 0.55 + fy.abs();
                if d > 1.0 { return image::Rgba([0, 0, 0, 0]); }
                let lit = if fx + fy < 0.0 { 255 } else { 175 };
                image::Rgba([lit, lit, lit, 255])
            })
        }
        // A flame tongue: a teardrop, round at the bottom, tip up.
        FLAME => {
            let (w, h) = (48u32, 96u32);
            image::RgbaImage::from_fn(w, h, |x, y| {
                let fx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
                let fy = (y as f32 + 0.5) / h as f32; // 0 top .. 1 bottom
                let half = (fy.powf(0.8) * (1.0 - fy).max(0.0).powf(0.25) * 1.5).min(1.0);
                let d = if half > 0.0 { fx.abs() / half } else { 2.0 };
                // Feathered all round, faint at the tip, strongest low.
                let a = (1.0 - d).clamp(0.0, 1.0).powf(1.6) * (fy * 2.0).min(1.0).powf(1.5);
                image::Rgba([255, 255, 255, (a * 255.0) as u8])
            })
        }
        // A wide, faint halo.
        HALO => radial(128, &[(0.0, [255.0, 255.0, 255.0, 160.0]), (0.4, [255.0, 255.0, 255.0, 70.0]), (1.0, [255.0, 255.0, 255.0, 0.0])]),
        // A four-point glint.
        STAR => {
            let n = 64u32;
            image::RgbaImage::from_fn(n, n, |x, y| {
                let (fx, fy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                let cross = (1.0 - fx.abs()).powf(6.0) * (1.0 - fy.abs() * 8.0).max(0.0)
                    + (1.0 - fy.abs()).powf(6.0) * (1.0 - fx.abs() * 8.0).max(0.0);
                let core = (1.0 - (fx * fx + fy * fy).sqrt() * 3.0).max(0.0);
                let a = (cross + core).min(1.0);
                image::Rgba([255, 255, 255, (a * 255.0) as u8])
            })
        }
        // An eighth note: a head and a stem with a flag.
        NOTE => {
            let (w, h) = (48u32, 64u32);
            image::RgbaImage::from_fn(w, h, |x, y| {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let head = (((fx - 17.0) / 13.0).powi(2) + ((fy - 51.0) / 9.0).powi(2)) <= 1.0;
                let stem = (27.0..31.0).contains(&fx) && (6.0..51.0).contains(&fy);
                let flag = fy >= 6.0 && fy <= 26.0 && fx >= 29.0 && fx <= 29.0 + (fy - 6.0) * 0.8 + 4.0
                    && fx >= 29.0 + (fy - 6.0) * 0.8 - 4.0;
                if head || stem || flag { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([0, 0, 0, 0]) }
            })
        }
        _ => image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0])),
    }
}

/// The shared particle shapes, by index (`DOT`, `SPARK`, ...).
pub fn img(i: usize) -> Img {
    static S: OnceLock<Vec<Img>> = OnceLock::new();
    S.get_or_init(|| (0..SHAPES).map(|i| Arc::new(shape(i))).collect())[i].clone()
}

/// Build every shape now (a background thread at app start).
pub fn prewarm() { let _ = img(0); }

// ── Objects ──────────────────────────────────────────────────────────────────

pub fn quad(name: &str, layer: i32) -> GameObject {
    let mut o = GameObject::build(name)
        .position(-9000.0, -9000.0)
        .size(8.0, 8.0)
        .image(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: img(DOT), color: None })
        .layer(layer)
        .finish();
    o.visible = false;
    o.gravity = 0.0;
    o.unlit = true;
    o
}

/// Make the named quad if it is not there yet.
pub fn ensure(c: &mut Canvas, name: &str, layer: i32) {
    if c.get_game_object(name).is_none() {
        c.add_game_object(name.to_string(), quad(name, layer));
    }
}

/// Place a quad centred at `at`, `size`, rotated, tinted (rgba multiplies).
pub fn put(c: &mut Canvas, name: &str, image: &Img, at: (f32, f32), size: (f32, f32), rot: f32, tint: [u8; 4]) {
    if let Some(o) = c.get_game_object_mut(name) {
        o.visible = tint[3] > 0 && size.0 > 0.5 && size.1 > 0.5;
        o.size = size;
        o.position = (at.0 - size.0 * 0.5, at.1 - size.1 * 0.5);
        o.rotation = rot;
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: image.clone(),
            color: Some(Color(tint[0], tint[1], tint[2], tint[3])) });
    }
}

pub fn hide(c: &mut Canvas, name: &str) {
    if let Some(o) = c.get_game_object_mut(name) { o.visible = false; }
}

/// A strip from `a` to `b`, `width` across.
pub fn strip(c: &mut Canvas, name: &str, image: &Img, a: (f32, f32), b: (f32, f32), width: f32, tint: [u8; 4]) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    put(c, name, image, ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5), (len, width), dy.atan2(dx).to_degrees(), tint);
}

/// `FX_TRACE=1`: say when an effect fires (for filming it headless).
pub fn trace(what: &str) {
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var("FX_TRACE").is_ok()) {
        eprintln!("fx: f{} {what}", FRAME.load(std::sync::atomic::Ordering::Relaxed));
    }
}

/// The headless driver's frame, for `trace`.
pub static FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// ── Randomness ───────────────────────────────────────────────────────────────

pub fn rnd(r: &mut u64) -> f32 {
    *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
    (*r >> 40) as f32 / (1u64 << 24) as f32
}

/// -1..1
pub fn rnds(r: &mut u64) -> f32 { rnd(r) * 2.0 - 1.0 }

// ── Particles ────────────────────────────────────────────────────────────────

/// One particle. `rgb` is its colour at birth, `rgb_end` at death.
#[derive(Clone, Copy)]
pub struct P {
    pub x: f32, pub y: f32, pub vx: f32, pub vy: f32,
    pub life: f32, pub max: f32,
    /// Size at birth; `grow` is how much it grows (x) by death.
    pub w: f32, pub h: f32, pub grow: f32,
    pub rot: f32, pub spin: f32,
    /// Per-tick velocity kept (1 = none lost), and downward pull.
    pub drag: f32, pub grav: f32,
    pub shape: usize,
    pub rgb: [u8; 3], pub rgb_end: [u8; 3],
    pub alpha: f32,
    /// Point along the velocity (streaks).
    pub align: bool,
    /// Fraction of its life spent fading in.
    pub fade_in: f32,
}

impl Default for P {
    fn default() -> Self {
        P { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, life: 30.0, max: 30.0, w: 20.0, h: 20.0, grow: 0.0,
            rot: 0.0, spin: 0.0, drag: 1.0, grav: 0.0, shape: DOT, rgb: [255; 3], rgb_end: [255; 3],
            alpha: 1.0, align: false, fade_in: 0.08 }
    }
}

impl P {
    pub fn at(x: f32, y: f32) -> Self { P { x, y, ..Default::default() } }
    pub fn vel(mut self, vx: f32, vy: f32) -> Self { self.vx = vx; self.vy = vy; self }
    pub fn life(mut self, t: f32) -> Self { self.life = t; self.max = t; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.w = w; self.h = h; self }
    pub fn grow(mut self, g: f32) -> Self { self.grow = g; self }
    pub fn rot(mut self, r: f32, spin: f32) -> Self { self.rot = r; self.spin = spin; self }
    pub fn drag(mut self, d: f32) -> Self { self.drag = d; self }
    pub fn grav(mut self, g: f32) -> Self { self.grav = g; self }
    pub fn shape(mut self, s: usize) -> Self { self.shape = s; self }
    pub fn rgb(mut self, a: [u8; 3], b: [u8; 3]) -> Self { self.rgb = a; self.rgb_end = b; self }
    pub fn alpha(mut self, a: f32) -> Self { self.alpha = a; self }
    pub fn align(mut self) -> Self { self.align = true; self }
    pub fn fade_in(mut self, f: f32) -> Self { self.fade_in = f; self }
}

/// A fixed set of particle objects, `<prefix>_<i>`, on one layer. Emitting
/// past the cap drops the particle (the oldest ones are mid-fade anyway).
pub struct Pool {
    pub prefix: &'static str,
    pub cap: usize,
    pub layer: i32,
    pub ps: Vec<P>,
    /// Art of the boss's own, drawn by shape `EXTRA + i`.
    pub extra: Vec<Img>,
    made: bool,
    shown: usize,
}

impl Pool {
    pub const fn new(prefix: &'static str, cap: usize, layer: i32) -> Self {
        Pool { prefix, cap, layer, ps: Vec::new(), extra: Vec::new(), made: false, shown: 0 }
    }

    pub fn emit(&mut self, p: P) {
        // Lower graphics settings keep a share of the particles.
        if self.ps.len() < self.cap && crate::graphics::keep_particle() { self.ps.push(p); }
    }

    pub fn is_empty(&self) -> bool { self.ps.is_empty() }

    pub fn step(&mut self) {
        for p in self.ps.iter_mut() {
            p.vx *= p.drag; p.vy *= p.drag;
            p.vy += p.grav;
            p.x += p.vx; p.y += p.vy;
            p.rot += p.spin;
            p.life -= 1.0;
        }
        self.ps.retain(|p| p.life > 0.0);
    }

    pub fn draw(&mut self, c: &mut Canvas) {
        if !self.made {
            self.made = true;
            for i in 0..self.cap { ensure(c, &format!("{}_{i}", self.prefix), self.layer); }
        }
        for (i, p) in self.ps.iter().enumerate() {
            let k = (p.life / p.max).clamp(0.0, 1.0); // 1 at birth .. 0 at death
            let age = 1.0 - k;
            let a = p.alpha * (age / p.fade_in.max(1e-3)).min(1.0) * (k / 0.45).min(1.0);
            let g = 1.0 + p.grow * age;
            let col = [0, 1, 2].map(|j| (p.rgb[j] as f32 + (p.rgb_end[j] as f32 - p.rgb[j] as f32) * age) as u8);
            let rot = if p.align { p.vy.atan2(p.vx).to_degrees() } else { p.rot };
            let art = if p.shape >= EXTRA { self.extra[p.shape - EXTRA].clone() } else { img(p.shape) };
            put(c, &format!("{}_{i}", self.prefix), &art, (p.x, p.y), (p.w * g, p.h * g), rot,
                [col[0], col[1], col[2], (a.clamp(0.0, 1.0) * 255.0) as u8]);
        }
        for i in self.ps.len()..self.shown { hide(c, &format!("{}_{i}", self.prefix)); }
        self.shown = self.ps.len();
    }

    /// Drop everything and hide it.
    pub fn clear(&mut self, c: &mut Canvas) {
        self.ps.clear();
        if self.made { self.draw(c); }
    }
}

// ── Lightning ────────────────────────────────────────────────────────────────

/// A jagged line from `a` to `b` in `n` segments, each point thrown sideways
/// by up to `jag` (the ends stay put). `seed` picks the shape: change it every
/// few ticks and the bolt crackles.
pub fn bolt_path(a: (f32, f32), b: (f32, f32), n: usize, jag: f32, seed: &mut u64) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    (0..=n).map(|i| {
        let t = i as f32 / n as f32;
        let off = if i == 0 || i == n { 0.0 } else { rnds(seed) * jag * (1.0 - (2.0 * t - 1.0).powi(2) * 0.6) };
        (a.0 + dx * t + nx * off, a.1 + dy * t + ny * off)
    }).collect()
}

/// Segments drawn as quads, `<prefix>_<i>`: a bright thin core over a wider,
/// fainter glow (two quads per segment).
pub struct Bolts {
    pub prefix: &'static str,
    pub cap: usize,
    pub layer: i32,
    made: bool,
    shown: usize,
}

impl Bolts {
    pub const fn new(prefix: &'static str, cap: usize, layer: i32) -> Self {
        Bolts { prefix, cap, layer, made: false, shown: 0 }
    }

    /// Draw these polylines (each with its width and colour) this frame.
    pub fn draw(&mut self, c: &mut Canvas, lines: &[(Vec<(f32, f32)>, f32, [u8; 4])]) {
        if !self.made {
            self.made = true;
            for i in 0..self.cap * 2 { ensure(c, &format!("{}_{i}", self.prefix), self.layer); }
        }
        let core = core_img();
        let glow = img(HALO);
        let mut k = 0;
        for (pts, w, col) in lines {
            for seg in pts.windows(2) {
                if k + 2 > self.cap * 2 { break; }
                strip(c, &format!("{}_{k}", self.prefix), &glow, seg[0], seg[1], w * 4.0,
                      [col[0], col[1], col[2], (col[3] as f32 * 0.45) as u8]);
                strip(c, &format!("{}_{}", self.prefix, k + 1), &core, seg[0], seg[1], *w, *col);
                k += 2;
            }
        }
        for i in k..self.shown { hide(c, &format!("{}_{i}", self.prefix)); }
        self.shown = k;
    }
}

/// A bolt's core across its width: white middle, soft sides.
fn core_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || band(&[(0.0, [255.0; 4]), (0.35, [255.0, 255.0, 255.0, 230.0]), (1.0, [255.0, 255.0, 255.0, 0.0])]))
}

/// A glow across a strip's width: faint, falling away smoothly to nothing —
/// light ON a line, not a band of colour.
pub fn glow_band() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let (w, h) = (2048u32, 32u32);
        image::RgbaImage::from_fn(w, h, |_, y| {
            let d = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
            let a = (1.0 - d).powf(2.4);
            image::Rgba([255, 255, 255, (a * 255.0) as u8])
        })
    })
}
