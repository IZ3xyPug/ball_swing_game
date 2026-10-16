//! The SKY planets at the sky's own resolution (3840x2160), made around the
//! original horizon photo (`assets/aurora_earth.gif`).
//!
//! The photo gives each world its framing and its air: space above, the glow
//! of an atmosphere over a horizon 82.5% of the way down (smoothed, so the
//! GIF's colour banding is gone). Below the horizon the ground is rendered,
//! not painted: a camera one unit above a ground plane looks out at it, so
//! the surface recedes in true perspective; the surface is a height field lit
//! by a low sun (soft light, shadowed slopes, specular where it is wet or
//! icy); detail fades out with distance before it can shimmer; and the air
//! thickens over distance into the world's own haze.
//!
//!     cargo run --release --example make_planets
//!
//! Writes `assets/planets/<name>.jpg`, `assets/planets/sheet.jpg`, and the
//! OCEAN WORLD's water animation, `assets/planets/ocean_water_<nn>.jpg`: the
//! sea below the horizon, frame by frame through one loop of its swell.

use image::{Rgb, RgbImage};

const W: usize = 3840;
const H: usize = 2160;
/// The photo's horizon, as a share of its height.
const HORIZON: f32 = 0.825;
/// Focal length in px (the camera's field of view).
const FOCAL: f32 = H as f32;
/// Water animation: frames in one loop, and the first row of the band they
/// cover (just below the horizon's blend into the air).
pub const WATER_FRAMES: usize = 16;
const WATER_BAND_TOP: usize = (H as f32 * HORIZON) as usize + 8;
/// The animation frames are kept at this width (the band's own height scales).
const WATER_FRAME_W: u32 = 2560;

type C = [f32; 3];

// ── Small maths ──────────────────────────────────────────────────────────────

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix(a: C, b: C, t: f32) -> C {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn add(a: C, b: C, k: f32) -> C { [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k] }
fn mul(a: C, k: f32) -> C { [a[0] * k, a[1] * k, a[2] * k] }
fn mulc(a: C, b: C) -> C { [a[0] * b[0] / 255.0, a[1] * b[1] / 255.0, a[2] * b[2] / 255.0] }
fn ramp(stops: &[C], t: f32) -> C {
    let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (t.floor() as usize).min(stops.len() - 2);
    mix(stops[i], stops[i + 1], t - i as f32)
}
fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}
fn value(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    // Quintic fade: no visible grid in the derivatives (the lighting).
    let s = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (sx, sy) = (s(fx), s(fy));
    let a = hash(xi, yi, seed) + (hash(xi + 1, yi, seed) - hash(xi, yi, seed)) * sx;
    let b = hash(xi, yi + 1, seed) + (hash(xi + 1, yi + 1, seed) - hash(xi, yi + 1, seed)) * sx;
    a + (b - a) * sy
}
/// Fractal noise that leaves out the octaves finer than `fp` (the size of a
/// pixel on the ground there), fading the last one in, so distant ground is
/// smooth rather than a shimmer of aliased detail.
fn fbm(x: f32, y: f32, seed: u32, oct: u32, fp: f32) -> f32 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..oct {
        let wave = 1.0 / f;
        let k = smooth(fp * 1.5, fp * 4.0, wave);
        if k <= 0.0 { break; }
        sum += (value(x * f, y * f, seed + o * 17) - 0.5) * amp * k;
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    0.5 + sum / norm.max(1e-6)
}
/// Domain-warped: features that flow and branch like real terrain.
fn warped(x: f32, y: f32, seed: u32, oct: u32, fp: f32, amt: f32) -> f32 {
    let wx = fbm(x * 0.7 + 3.1, y * 0.7, seed + 101, 4, fp);
    let wy = fbm(x * 0.7, y * 0.7 + 7.7, seed + 202, 4, fp);
    fbm(x + (wx - 0.5) * amt, y + (wy - 0.5) * amt, seed, oct, fp)
}
/// Sharp crests: 1 at the noise's mid-line, falling off to either side.
fn ridged(x: f32, y: f32, seed: u32, oct: u32, fp: f32) -> f32 {
    1.0 - (fbm(x, y, seed, oct, fp) - 0.5).abs() * 2.0
}

// ── The canvas ───────────────────────────────────────────────────────────────

struct Img { px: Vec<C> }
impl Img {
    fn new() -> Img { Img { px: vec![[0.0; 3]; W * H] } }
    fn get(&self, x: usize, y: usize) -> C { self.px[y * W + x] }
    fn set(&mut self, x: usize, y: usize, c: C) { self.px[y * W + x] = c; }
    fn over(&mut self, x: usize, y: usize, c: C, a: f32) {
        if x >= W || y >= H || a <= 0.0 { return; }
        let p = self.get(x, y);
        self.set(x, y, mix(p, c, a));
    }
    fn glow(&mut self, x: usize, y: usize, c: C, k: f32) {
        if x >= W || y >= H || k <= 0.0 { return; }
        let p = self.get(x, y);
        self.set(x, y, add(p, c, k));
    }
    fn bloom(&mut self, cx: f32, cy: f32, r: f32, c: C, k: f32) {
        let (x0, x1) = ((cx - r).max(0.0) as usize, ((cx + r) as usize).min(W - 1));
        let (y0, y1) = ((cy - r).max(0.0) as usize, ((cy + r) as usize).min(H - 1));
        for y in y0..=y1 { for x in x0..=x1 {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
            if d < 1.0 { self.glow(x, y, c, k * (1.0 - d).powi(2)); }
        }}
    }
    /// To 8-bit, with a whisper of noise so smooth gradients do not band.
    fn to_rgb(&self, rows: std::ops::Range<usize>) -> RgbImage {
        let mut out = RgbImage::new(W as u32, rows.len() as u32);
        for (oy, y) in rows.enumerate() {
            for x in 0..W {
                let p = self.get(x, y);
                let d = (hash(x as i32, y as i32, 777) - 0.5) * 1.6;
                out.put_pixel(x as u32, oy as u32, Rgb([(p[0] + d).round().clamp(0.0, 255.0) as u8,
                    (p[1] + d).round().clamp(0.0, 255.0) as u8, (p[2] + d).round().clamp(0.0, 255.0) as u8]));
            }
        }
        out
    }
}

fn save_jpg(img: &RgbImage, path: &std::path::Path, q: u8) {
    let f = std::fs::File::create(path).expect("create");
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), q);
    enc.encode_image(img).expect("encode");
}

/// The photo's brightness at full size, smoothed (the GIF's palette banded
/// the glow into visible steps).
fn photo_lum() -> Vec<f32> {
    let src = image::load_from_memory(include_bytes!("../assets/aurora_earth.gif")).expect("photo").to_rgba8();
    let big = image::imageops::resize(&src, W as u32, H as u32, image::imageops::FilterType::Lanczos3);
    // The air is all smooth gradient: blur it wide enough to melt the GIF's
    // colour steps (each is tens of pixels across at this size).
    let big = image::imageops::blur(&big, 26.0);
    big.pixels().map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0).collect()
}

// ── The camera and the ground ────────────────────────────────────────────────

fn horizon_px() -> f32 { H as f32 * HORIZON }

/// The horizon at column `x` for a world whose limb drops by `curve` px at
/// the picture's far edges: one gentle arc across the whole width, as a
/// planet's limb is, so it only reads once the sky zooms out to show its
/// edges.
fn horizon_at(x: usize, curve: f32) -> f32 {
    let u = (x as f32 + 0.5) / W as f32 * 2.0 - 1.0;
    horizon_px() + curve * u * u
}

fn ground_at(x: usize, y: usize) -> (f32, f32, f32) { ground_at_h(x, y, horizon_px()) }

fn ground_at_h(x: usize, y: usize, hz: f32) -> (f32, f32, f32) {
    let below = (y as f32 + 0.5 - hz).max(0.35);
    let z = FOCAL / below;
    let gx = (x as f32 + 0.5 - W as f32 * 0.5) * z / FOCAL;
    // A row covers dz = z^2 / FOCAL of ground; a column z / FOCAL.
    let fp = (z * z / FOCAL).max(z / FOCAL);
    (gx, z, fp)
}

/// The sun: low, ahead and to the right (where the photo's glow is brightest).
fn sun_dir() -> [f32; 3] { norm3([0.30, 0.26, 0.78]) }
/// The ray from the camera through a ground pixel (x right, y up, z away).
fn view_dir(gx: f32, z: f32) -> [f32; 3] { norm3([gx, -1.0, z]) }

/// A world's look.
struct World {
    name: &'static str,
    /// The photo's air glow mapped through these (dark space .. bright limb).
    air: [C; 4],
    /// The colour the ground fades into with distance.
    haze: C,
    /// How quickly it thickens (bigger is murkier).
    haze_k: f32,
    sun: C,
    sky_light: C,
    /// How far the limb drops at the far edges (see `horizon_at`).
    curve: f32,
}

/// Light a surface point: `albedo` under the sun and the sky, normal `n`,
/// plus a highlight of `spec` (0 for matte) sharp to `shine`.
fn light(w: &World, albedo: C, n: [f32; 3], view: [f32; 3], spec: f32, shine: f32) -> C {
    let s = sun_dir();
    let lambert = dot(n, s).max(0.0);
    // Wrap the light a little: real ground scatters it round its slopes.
    let diffuse = (lambert * 0.8 + 0.2 * (dot(n, s) * 0.5 + 0.5)).max(0.0);
    let sky = 0.35 + 0.25 * n[1];
    let mut c = add(mul(mulc(albedo, w.sun), diffuse * 1.15), mulc(albedo, w.sky_light), sky);
    if spec > 0.0 {
        let r = { let d = dot(view, n) * 2.0; norm3([view[0] - n[0] * d, view[1] - n[1] * d, view[2] - n[2] * d]) };
        let hl = dot(r, s).max(0.0).powf(shine);
        c = add(c, w.sun, hl * spec);
    }
    c
}

/// A height field's normal at (x, z), from its slope (`h` in camera units).
fn normal_of(h: &dyn Fn(f32, f32) -> f32, x: f32, z: f32, e: f32) -> [f32; 3] {
    let hx = (h(x + e, z) - h(x - e, z)) / (2.0 * e);
    let hz = (h(x, z + e) - h(x, z - e)) / (2.0 * e);
    norm3([-hx, 1.0, -hz])
}

/// Paint the air from the photo, then the ground below the horizon with
/// `ground(x, y, gx, z, fp) -> colour`, haze over it, blended into the air
/// across the horizon.
fn render(w: &World, lum: &[f32], ground: &(dyn Fn(usize, usize, f32, f32, f32) -> C + Sync)) -> Img {
    let mut img = Img::new();
    for y in 0..H { for x in 0..W { img.set(x, y, ramp(&w.air, lum[y * W + x])); } }
    let hz = horizon_px();
    let rows: Vec<usize> = ((hz - 10.0) as usize..H).collect();
    // Rows are independent: render them on every core.
    let threads = std::thread::available_parallelism().map_or(8, |n| n.get());
    let chunks: Vec<Vec<usize>> = rows.chunks((rows.len() + threads - 1) / threads).map(|c| c.to_vec()).collect();
    let results: Vec<Vec<(usize, Vec<C>)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|chunk| sc.spawn(move || {
            chunk.iter().map(|&y| (y, (0..W).map(|x| {
                let (gx, z, fp) = ground_at_h(x, y, horizon_at(x, w.curve));
                let c = ground(x, y, gx, z, fp);
                // Aerial perspective.
                let fog = 1.0 - (-z * w.haze_k).exp();
                mix(c, w.haze, fog)
            }).collect())).collect()
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for part in results { for (y, row) in part {
        for (x, c) in row.into_iter().enumerate() {
            let hzx = horizon_at(x, w.curve);
            let a = smooth(hzx - 8.0, hzx + 6.0, y as f32);
            let p = img.get(x, y);
            img.set(x, y, mix(p, c, a));
        }
    }}
    img
}

// ── The worlds ───────────────────────────────────────────────────────────────

fn gas_giant(lum: &[f32]) -> Img {
    let w = World { name: "gas_giant",
        air: [[0.0; 3], [104.0, 66.0, 38.0], [212.0, 168.0, 118.0], [255.0, 238.0, 204.0]],
        haze: [214.0, 172.0, 124.0], haze_k: 0.0035, sun: [255.0, 236.0, 206.0], sky_light: [200.0, 160.0, 120.0], curve: 45.0 };
    let mut img = render(&w, lum, &|_x, _y, gx, z, fp| {
        // The cloud deck: belts across, sheared and curling at their edges.
        let shear = warped(gx * 0.05, z * 0.02, 51, 6, fp * 0.05, 3.0);
        let belt = 0.5 + 0.5 * (z * 0.09 + shear * 6.0).sin();
        // Eddies along the belts' edges.
        let eddy = warped(gx * 0.18, z * 0.25, 54, 7, fp * 0.18, 4.0);
        let belt = (belt * 0.75 + (eddy - 0.5) * 0.9 + 0.12).clamp(0.0, 1.0);
        let tops = |x: f32, z: f32| warped(x * 0.35, z * 0.6, 52, 8, fp * 0.35, 1.6) * 0.25;
        let n = normal_of(&tops, gx, z, 0.05 + fp);
        let albedo = ramp(&[[96.0, 52.0, 32.0], [166.0, 100.0, 60.0], [222.0, 170.0, 116.0], [250.0, 238.0, 214.0]],
            belt * 0.85 + tops(gx, z) * 0.9);
        // A great storm: an oval vortex out in the deck.
        let (sx, sz) = ((gx - 4.0) / 3.2, (z - 14.0) / 4.5);
        let r = (sx * sx + sz * sz).sqrt();
        let albedo = if r < 1.0 {
            let ang = sz.atan2(sx) + r * 5.0;
            let swirl = 0.5 + 0.5 * (ang * 3.0 + fbm(sx * 3.0, sz * 3.0, 53, 4, 0.0) * 3.0).sin();
            mix(albedo, mix([170.0, 72.0, 46.0], [236.0, 170.0, 130.0], swirl), smooth(1.0, 0.6, r))
        } else { albedo };
        light(&w, albedo, n, view_dir(gx, z), 0.0, 1.0)
    });
    // The ring, edge-on: an arc rising out of the horizon, ringlets and a gap.
    let hz = horizon_px();
    for x in 0..W {
        let u = x as f32 / W as f32;
        let centre = hz + 60.0 - 0.32 * H as f32 * (1.0 - ((u - 0.80) / 0.95).powi(2));
        let thick = 24.0 + 44.0 * u;
        for y in ((centre - thick * 1.2).max(0.0) as usize)..((centre + thick * 1.2) as usize).min(hz as usize) {
            let t = (y as f32 - centre) / thick;
            if t.abs() > 1.0 { continue; }
            let ringlets = 0.6 + 0.4 * ((t * 47.0).sin() * 0.5 + (t * 13.0 + 1.0).cos() * 0.5);
            let gap = 1.0 - smooth(0.05, 0.0, (t - 0.25).abs()) * 0.92;
            let edge = smooth(1.0, 0.8, t.abs());
            img.over(x, y, mix([196.0, 168.0, 128.0], [255.0, 242.0, 214.0], 1.0 - t.abs()), edge * gap * ringlets * 0.8);
        }
    }
    img
}

fn ice(lum: &[f32]) -> Img {
    let w = World { name: "ice",
        air: [[0.0; 3], [26.0, 72.0, 112.0], [128.0, 196.0, 232.0], [232.0, 248.0, 255.0]],
        haze: [176.0, 214.0, 238.0], haze_k: 0.0045, sun: [255.0, 250.0, 240.0], sky_light: [150.0, 190.0, 235.0], curve: 0.0 };
    render(&w, lum, &|x, y, gx, z, fp| {
        // Snow plains, wind-carved, pressure ridges running across.
        let h = |x: f32, z: f32| {
            let plains = warped(x * 0.08, z * 0.08, 61, 7, fp * 0.08, 2.5) * 0.9;
            let ridges = ridged(x * 0.05, z * 0.11, 62, 5, fp * 0.05).powf(6.0) * 0.6;
            let sastrugi = fbm(x * 2.5, z * 0.5, 63, 5, fp * 2.5) * 0.06;
            plains + ridges + sastrugi
        };
        let n = normal_of(&h, gx, z, 0.03 + fp);
        let mut albedo = mix([214.0, 232.0, 248.0], [250.0, 252.0, 255.0], fbm(gx * 0.3, z * 0.3, 64, 5, fp * 0.3));
        // Crevasses: deep blue cuts.
        let crack = smooth(0.965, 0.995, ridged(gx * 0.03, z * 0.03, 65, 6, fp * 0.03));
        albedo = mix(albedo, [30.0, 96.0, 170.0], crack);
        let view = view_dir(gx, z);
        let mut c = light(&w, albedo, n, view, 0.35, 60.0);
        // Glints: snow crystals catching the sun, near the sun's path.
        let path = (-((x as f32 / W as f32 - 0.70) / 0.18).powi(2)).exp();
        if hash(x as i32, y as i32, 66) > 0.997 - 0.004 * path { c = add(c, [255.0; 3], 0.6 * (1.0 - fp.min(1.0))); }
        // Shadowed snow goes blue.
        let shade = 1.0 - dot(n, sun_dir()).max(0.0);
        mix(c, mulc(c, [150.0, 190.0, 255.0]), shade * 0.35)
    })
}

/// The sea's surface: swells from several directions (each completing whole
/// cycles over the animation's loop, so it loops), and chop on top. `t` is
/// 0..1 through the loop.
fn sea_height(x: f32, z: f32, t: f32, fp: f32) -> f32 {
    const WAVES: [(f32, f32, f32, f32); 9] = [
        // (direction, wavelength, height, cycles per loop)
        (0.20, 9.0, 0.050, 1.0), (-0.45, 6.5, 0.035, 1.0), (0.90, 4.6, 0.025, 2.0),
        (-1.10, 3.2, 0.018, 2.0), (0.55, 2.2, 0.012, 3.0), (1.60, 1.6, 0.009, 3.0),
        (-0.15, 1.1, 0.006, 4.0), (2.30, 0.75, 0.004, 5.0), (-2.00, 0.5, 0.003, 6.0),
    ];
    let mut h = 0.0;
    for (i, &(dir, len, amp, cyc)) in WAVES.iter().enumerate() {
        // Waves shorter than a pixel there are averaged away (no shimmer).
        let k = smooth(fp * 1.5, fp * 4.0, len);
        if k <= 0.0 { continue; }
        let (dx, dz) = (dir.cos(), dir.sin());
        let ph = (x * dx + z * dz) / len * std::f32::consts::TAU - t * cyc * std::f32::consts::TAU + i as f32 * 1.7;
        // Sharpened crests (Gerstner-like): peaks narrow, troughs broad.
        let s = ph.sin();
        h += amp * k * (s - 0.25 * s * s);
    }
    h
}

fn ocean_world() -> World {
    World { name: "ocean",
        air: [[0.0; 3], [14.0, 44.0, 116.0], [70.0, 140.0, 218.0], [200.0, 228.0, 255.0]],
        haze: [120.0, 170.0, 225.0], haze_k: 0.0028, sun: [255.0, 244.0, 222.0], sky_light: [120.0, 170.0, 230.0], curve: 0.0 }
}

/// The sea at one moment of its loop.
fn ocean_pixel(w: &World, x: usize, y: usize, gx: f32, z: f32, fp: f32, t: f32) -> C {
    let h = |a: f32, b: f32| sea_height(a, b, t, fp);
    let n = normal_of(&h, gx, z, 0.02 + fp * 0.5);
    let view = view_dir(gx, z);
    // Fresnel: grazing angles mirror the sky, steep ones show the deep water.
    let cos_i = (-dot(view, n)).clamp(0.0, 1.0);
    let fresnel = 0.02 + 0.98 * (1.0 - cos_i).powi(5);
    let deep = mix([2.0, 16.0, 40.0], [8.0, 46.0, 92.0], smooth(-0.04, 0.06, h(gx, z)));
    let sky = mix([150.0, 196.0, 240.0], [90.0, 140.0, 210.0], cos_i);
    let mut c = mix(deep, sky, fresnel);
    // The sun on the water: a glitter path of sharp highlights.
    let r = { let d = dot(view, n) * 2.0; norm3([view[0] - n[0] * d, view[1] - n[1] * d, view[2] - n[2] * d]) };
    let sd = dot(r, sun_dir()).max(0.0);
    c = add(c, w.sun, sd.powf(900.0) * 3.5 + sd.powf(60.0) * 0.25);
    // Whitecaps on the steepest crests, near enough to see.
    let steep = 1.0 - n[1];
    let foam = smooth(0.06, 0.12, steep) * smooth(0.5, 0.75, fbm(gx * 0.8, z * 0.8, 81, 4, fp * 0.8)) * (1.0 - smooth(20.0, 60.0, z));
    c = mix(c, [235.0, 242.0, 250.0], foam * 0.8);
    // Scattered cloud shadows drifting on the sea (fixed: they move slowly).
    let shadow = smooth(0.55, 0.7, warped(gx * 0.04, z * 0.04, 82, 5, fp * 0.04, 2.0));
    let _ = (x, y);
    mul(c, 1.0 - shadow * 0.35)
}

fn ocean(lum: &[f32]) -> Img {
    let w = ocean_world();
    render(&w, lum, &|x, y, gx, z, fp| ocean_pixel(&w, x, y, gx, z, fp, 0.0))
}

/// The sea's band, frame by frame (the static picture is frame 0).
fn ocean_frames(dir: &std::path::Path) {
    let w = ocean_world();
    let rows: Vec<usize> = (WATER_BAND_TOP..H).collect();
    let fh = (WATER_FRAME_W as f32 * rows.len() as f32 / W as f32).round() as u32;
    for f in 0..WATER_FRAMES {
        let t = f as f32 / WATER_FRAMES as f32;
        let threads = std::thread::available_parallelism().map_or(8, |n| n.get());
        let chunks: Vec<Vec<usize>> = rows.chunks((rows.len() + threads - 1) / threads).map(|c| c.to_vec()).collect();
        let parts: Vec<Vec<(usize, Vec<C>)>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunks.iter().map(|chunk| { let w = &w; sc.spawn(move || {
                chunk.iter().map(|&y| (y, (0..W).map(|x| {
                    let (gx, z, fp) = ground_at(x, y);
                    let c = ocean_pixel(w, x, y, gx, z, fp, t);
                    mix(c, w.haze, 1.0 - (-z * w.haze_k).exp())
                }).collect())).collect()
            })}).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut band = RgbImage::new(W as u32, rows.len() as u32);
        for part in parts { for (y, row) in part { for (x, c) in row.into_iter().enumerate() {
            band.put_pixel(x as u32, (y - WATER_BAND_TOP) as u32, Rgb([c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8]));
        }}}
        let small = image::imageops::resize(&band, WATER_FRAME_W, fh, image::imageops::FilterType::Lanczos3);
        save_jpg(&small, &dir.join(format!("ocean_water_{f:02}.jpg")), 88);
    }
    println!("assets/planets/ocean_water_00..{:02}.jpg  ({WATER_FRAME_W}x{fh}, from row {WATER_BAND_TOP})", WATER_FRAMES - 1);
}

fn toxic(lum: &[f32]) -> Img {
    let w = World { name: "toxic",
        air: [[0.0; 3], [44.0, 60.0, 8.0], [148.0, 172.0, 42.0], [226.0, 238.0, 146.0]],
        haze: [168.0, 186.0, 82.0], haze_k: 0.0085, sun: [240.0, 250.0, 190.0], sky_light: [160.0, 180.0, 90.0], curve: 0.0 };
    let mut img = render(&w, lum, &|_x, _y, gx, z, fp| {
        // Eroded flats, and pools of acid lying in the low ground.
        let h = |x: f32, z: f32| warped(x * 0.07, z * 0.07, 91, 8, fp * 0.07, 3.0);
        let level = h(gx, z);
        let pool = smooth(0.40, 0.37, level);
        let n = if pool > 0.5 { [0.0, 1.0, 0.0] } else { normal_of(&|a, b| h(a, b) * 0.8, gx, z, 0.04 + fp) };
        let ground = ramp(&[[36.0, 34.0, 18.0], [82.0, 86.0, 34.0], [138.0, 140.0, 66.0], [176.0, 170.0, 110.0]],
            level * 0.8 + fbm(gx * 0.8, z * 0.8, 92, 6, fp * 0.8) * 0.3);
        let view = view_dir(gx, z);
        let mut c = light(&w, ground, n, view, 0.0, 1.0);
        // The pools: glowing from within, a pale crust round their edge.
        let rim = smooth(0.43, 0.40, level) - pool;
        c = mix(c, [220.0, 226.0, 170.0], rim.max(0.0) * 0.5);
        let glow = mix([60.0, 200.0, 40.0], [190.0, 255.0, 120.0], smooth(0.37, 0.30, level));
        mix(c, add(glow, w.sun, 0.05), pool)
    });
    // The sun, dim and swollen through the haze.
    img.bloom(0.68 * W as f32, 0.69 * H as f32, 0.22 * H as f32, [170.0, 196.0, 80.0], 0.4);
    img.bloom(0.68 * W as f32, 0.69 * H as f32, 0.034 * H as f32, [255.0, 255.0, 210.0], 0.9);
    img
}

fn desert(lum: &[f32]) -> Img {
    let w = World { name: "desert",
        air: [[0.0; 3], [92.0, 38.0, 18.0], [214.0, 132.0, 82.0], [255.0, 216.0, 172.0]],
        haze: [218.0, 150.0, 100.0], haze_k: 0.0055, sun: [255.0, 236.0, 206.0], sky_light: [210.0, 150.0, 120.0], curve: 0.0 };
    let mut img = render(&w, lum, &|_x, _y, gx, z, fp| {
        // Dunes: long crests (sharp on the lee, smooth on the windward side),
        // wandering, with ripples over everything.
        let h = |x: f32, z: f32| {
            let along = x * 0.32 + z * 0.12 + (warped(x * 0.03, z * 0.03, 101, 5, fp * 0.03, 2.0) - 0.5) * 6.0;
            let saw = along.rem_euclid(1.0);
            let dune = (1.0 - (saw - 0.72).abs() / 0.72).powf(1.6) * 0.55;
            let ripple = (x * 7.0 + z * 2.5 + fbm(x, z, 102, 3, fp) * 4.0).sin() * 0.008 * smooth(fp * 6.0, fp * 2.0, 0.15);
            dune + ripple + fbm(x * 0.2, z * 0.2, 103, 6, fp * 0.2) * 0.3
        };
        let n = normal_of(&h, gx, z, 0.03 + fp);
        let albedo = ramp(&[[150.0, 70.0, 34.0], [200.0, 116.0, 62.0], [232.0, 168.0, 104.0]],
            fbm(gx * 0.15, z * 0.15, 104, 5, fp * 0.15));
        light(&w, albedo, n, view_dir(gx, z), 0.0, 1.0)
    });
    // Twin moons, lit from the right.
    for (mx, my, mr, seed) in [(0.24, 0.64, 0.045, 105u32), (0.32, 0.585, 0.024, 106)] {
        let (mx, my, mr) = (mx * W as f32, my * H as f32, mr * H as f32);
        for y in ((my - mr) as usize)..((my + mr) as usize + 1) { for x in ((mx - mr) as usize)..((mx + mr) as usize + 1) {
            let (dx, dy) = ((x as f32 - mx) / mr, (y as f32 - my) / mr);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 1.0 { continue; }
            // A sphere's shading: lit side, terminator, and maria.
            let nz = (1.0 - d * d).max(0.0).sqrt();
            let lit = (dx * 0.8 - dy * 0.2 + nz * 0.35).max(0.0);
            let maria = fbm(dx * 3.0 + 9.0, dy * 3.0, seed, 6, 0.0);
            let c = mul([238.0, 222.0, 204.0], (0.62 + 0.38 * maria) * (0.04 + 0.96 * lit));
            img.over(x, y, c, smooth(1.0, 0.985, d) * 0.92);
        }}
    }
    img
}

fn violet_storm(lum: &[f32]) -> Img {
    let w = World { name: "violet_storm",
        air: [[0.0; 3], [44.0, 14.0, 86.0], [130.0, 70.0, 210.0], [232.0, 202.0, 255.0]],
        haze: [150.0, 110.0, 214.0], haze_k: 0.006, sun: [240.0, 220.0, 255.0], sky_light: [140.0, 100.0, 210.0], curve: 0.0 };
    let mut img = render(&w, lum, &|_x, _y, gx, z, fp| {
        // A storm deck: billowing cloud tops, deep shadow between them, and
        // lightning lighting them from inside here and there.
        let h = |x: f32, z: f32| warped(x * 0.12, z * 0.12, 111, 8, fp * 0.12, 2.6) * 0.8;
        let n = normal_of(&h, gx, z, 0.04 + fp);
        let albedo = ramp(&[[40.0, 22.0, 70.0], [110.0, 74.0, 170.0], [190.0, 156.0, 240.0]], h(gx, z) * 1.1);
        let mut c = light(&w, albedo, n, view_dir(gx, z), 0.0, 1.0);
        let flash = smooth(0.74, 0.86, fbm(gx * 0.04 + 3.0, z * 0.05, 112, 4, fp * 0.04));
        c = add(c, [200.0, 160.0, 255.0], flash * 0.5);
        c
    });
    // Thunderheads standing on the horizon, anvil tops, lit at the rim.
    let hz = horizon_px();
    for (cx, height, wide, seed) in [(0.15, 0.24, 0.07, 121u32), (0.46, 0.33, 0.10, 122), (0.80, 0.27, 0.08, 123)] {
        let cx = cx * W as f32;
        let top = hz - height * H as f32;
        let ww = wide * W as f32;
        for y in (top as usize).saturating_sub(90)..(hz as usize + 4) { for x in ((cx - ww * 3.0).max(0.0) as usize)..((cx + ww * 3.0) as usize).min(W) {
            let up = (hz - y as f32) / (hz - top);
            let anvil = smooth(0.68, 1.0, up) * 1.4;
            let r = ww * (0.55 + 0.25 * up + anvil);
            let dx = (x as f32 - cx) / r;
            if dx.abs() > 1.8 { continue; }
            let n = fbm((x as f32 - cx) / ww * 2.2, y as f32 / ww * 2.2, seed, 8, 0.0);
            let body = (1.0 - dx.abs()) + (n - 0.5) * 1.9 - smooth(0.92, 1.25, up + (n - 0.5) * 0.15) * 2.0;
            let a = smooth(0.0, 0.35, body);
            if a <= 0.0 { continue; }
            let rim = smooth(0.35, 0.0, body) * 0.8;
            let lit = smooth(-0.3, 0.6, dx) * 0.4;
            let c = mix(mix([30.0, 14.0, 56.0], [116.0, 76.0, 176.0], n + lit), [232.0, 196.0, 255.0], rim);
            img.over(x, y, c, a * 0.96);
        }}
        img.bloom(cx + ww * 0.2, hz - height * H as f32 * 0.45, ww * 0.6, [200.0, 170.0, 255.0], 0.6);
    }
    // A bolt, cloud to ground, jagged, branching, with its glow.
    let (mut bx, mut by) = (0.47 * W as f32, hz - 0.18 * H as f32);
    let mut step = 0u32;
    let mut branches: Vec<(f32, f32, u32)> = Vec::new();
    let leg = |img: &mut Img, (ax, ay): (f32, f32), (bx2, by2): (f32, f32), wdt: f32| {
        let n = (((bx2 - ax).powi(2) + (by2 - ay).powi(2)).sqrt() / 1.5) as usize + 1;
        for t in 0..=n {
            let f = t as f32 / n as f32;
            let (x, y) = (ax + (bx2 - ax) * f, ay + (by2 - ay) * f);
            img.bloom(x, y, 20.0 * wdt, [180.0, 150.0, 255.0], 0.08);
            img.bloom(x, y, 3.0 * wdt, [255.0, 250.0, 255.0], 0.9);
        }
    };
    while by < hz + 0.04 * H as f32 {
        let nx = bx + (hash(step as i32, 7, 124) - 0.5) * 100.0;
        let ny = by + 12.0 + hash(step as i32, 8, 124) * 32.0;
        leg(&mut img, (bx, by), (nx, ny), 1.0);
        if hash(step as i32, 9, 124) > 0.86 { branches.push((nx, ny, step)); }
        bx = nx; by = ny; step += 1;
    }
    for (mut x, mut y, s0) in branches {
        for k in 0..6u32 {
            let nx = x + (hash((s0 * 31 + k) as i32, 10, 125) - 0.3) * 70.0;
            let ny = y + 9.0 + hash((s0 * 31 + k) as i32, 11, 125) * 24.0;
            leg(&mut img, (x, y), (nx, ny), 0.6);
            x = nx; y = ny;
        }
    }
    img
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/planets");
    std::fs::create_dir_all(&dir).expect("dir");
    let lum = photo_lum();
    let worlds: [(&str, fn(&[f32]) -> Img); 6] = [
        ("gas_giant", gas_giant), ("ice", ice), ("ocean", ocean),
        ("toxic", toxic), ("desert", desert), ("violet_storm", violet_storm),
    ];
    let only = std::env::args().nth(1);
    let mut made = Vec::new();
    for (name, f) in worlds {
        if only.as_deref().is_some_and(|o| o != name) { continue; }
        let t = std::time::Instant::now();
        let img = f(&lum);
        let path = dir.join(format!("{name}.jpg"));
        save_jpg(&img.to_rgb(0..H), &path, 92);
        println!("assets/planets/{name}.jpg  {:.1}s  {} KB", t.elapsed().as_secs_f32(), std::fs::metadata(&path).map(|m| m.len() / 1024).unwrap_or(0));
        made.push(path);
    }
    if only.as_deref().map_or(true, |o| o == "ocean") { ocean_frames(&dir); }
    // The sheet: the lower 45% of each (where the planet is), two across.
    let (cw, ch) = (960u32, (960.0 * 0.45 * H as f32 / W as f32) as u32);
    let rows = (made.len() as u32 + 1) / 2;
    let mut sheet = RgbImage::from_pixel(cw * 2 + 24, (ch + 8) * rows + 8, Rgb([30, 30, 36]));
    for (k, p) in made.iter().enumerate() {
        let img = image::open(p).expect("open").to_rgb8();
        let crop = image::imageops::crop_imm(&img, 0, (H as f32 * 0.55) as u32, W as u32, (H as f32 * 0.45) as u32).to_image();
        let small = image::imageops::resize(&crop, cw, ch, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut sheet, &small, (8 + (k as u32 % 2) * (cw + 8)) as i64, (8 + (k as u32 / 2) * (ch + 8)) as i64);
    }
    sheet.save(dir.join("sheet.jpg")).expect("sheet");
}
