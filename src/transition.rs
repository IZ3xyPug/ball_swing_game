//! transition.rs — the shutter between screens.
//!
//! Leaving for the menu (or between menu pages) the screen CLOSES: two dark
//! panels with an angled seam slam together from the top and bottom, the seam
//! flashing where they meet. The next screen loads behind them, and it OPENS:
//! after a beat the panels part, easing out. Modelled on the closing/opening
//! wipes of Vex-style platformers.
//!
//! `go(c, scene)` runs the whole thing. It is driven by an `on_update_always`
//! callback, so it keeps moving over a paused game (the pause menu's MENU).

use quartz::*;
use std::sync::{Arc, OnceLock};

use crate::constants::*;

const TOP: &str = "tr_top";
const BOT: &str = "tr_bot";

/// Ticks to close, held closed before and after the load, and to open:
/// (between menu pages, leaving a run for the menu). Leaving a run is the
/// slower, weightier one.
const CLOSE_TICKS: [i32; 2] = [22, 36];
const HOLD_BEFORE: [i32; 2] = [5, 10];
const HOLD_AFTER: [i32; 2] = [8, 14];
const OPEN_TICKS: [i32; 2] = [30, 48];

fn speed(c: &Canvas) -> usize { c.get_bool("tr_slow") as usize }

/// How far the seam slants across the screen, and how far the panels are
/// drawn past the screen's edges.
const SLANT: f32 = 240.0;
const OVER_X: f32 = VW * 0.25;
const OVER_Y: f32 = VH * 0.5;

/// Phases (`tr_phase`).
const IDLE: i32 = 0;
const CLOSING: i32 = 1;
const OPENING: i32 = 2;
/// Shut, and staying shut until `toggle_hold` opens it again.
const HELD: i32 = 3;
/// `tr_next` for a close that holds rather than loading a scene.
const HOLD_SCENE: &str = "__hold";

fn panel_w() -> f32 { VW + OVER_X * 2.0 }
fn panel_h() -> f32 { VH * 0.5 + SLANT * 0.5 + OVER_Y }

/// The seam's height on screen at x (0..panel_w), closed.
fn seam_y(x: f32) -> f32 { VH * 0.5 + SLANT * 0.5 - SLANT * (x / panel_w()) }

/// A hash of a cell, 0..1.
fn hash2(i: i32, j: i32, salt: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x8da6_b343) ^ (j as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13; h = h.wrapping_mul(0x5bd1_e995); h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// What the door is at one point. `sx` is the screen x (the door is drawn
/// past both screen edges), `d` how far in from the seam.
struct Spot { h: f32, col: [f32; 3], glow: [f32; 3], lamp: bool }

const STEEL: [f32; 3] = [58.0, 74.0, 82.0];
const RED: [f32; 3] = [255.0, 40.0, 45.0];
const STRIP: [f32; 3] = [140.0, 215.0, 245.0];
/// The wedges outside each panel: the same dark grey as the recesses.
const ACCENT: [f32; 3] = [32.0, 42.0, 48.0];

fn add(g: &mut [f32; 3], c: [f32; 3], k: f32) { for i in 0..3 { g[i] += c[i] * k; } }

/// A chrome cylinder across its width: `u` -1..1 → (height, colour).
fn cyl(u: f32) -> (f32, [f32; 3]) {
    let curve = (1.0 - u * u).max(0.0).sqrt();
    let shine = (1.0 - ((u + 0.35) / 0.22).abs()).clamp(0.0, 1.0);
    let back = (1.0 - ((u - 0.55) / 0.2).abs()).clamp(0.0, 1.0) * 0.3;
    let v = 34.0 + 80.0 * curve + 150.0 * shine + 40.0 * back;
    (0.7 + 0.5 * curve, [v * 0.92, v * 1.0, v * 1.06])
}

/// Distance outside an axis-aligned box (0 inside).
fn box_dist(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let dx = (x0 - x).max(x - x1).max(0.0);
    let dy = (y0 - y).max(y - y1).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// A blast door: a central spine with red lock lights at the seam and
/// hydraulic pistons either side; hazard stripes along the seam; a recessed
/// panel each side, carrying a bank of fins, with dark accent
/// wedges outside. Hydraulic lines tie it together: out of the piston bays,
/// round each panel along its lit seam-side edge, into the accent. Laid out
/// from the seam, so it runs with the slant.
fn door_spot(sx: f32, d: f32) -> Spot {
    let mut sp = Spot { h: 1.0, col: STEEL, glow: [0.0; 3], lamp: false };
    let adx = (sx - VW * 0.5).abs();
    const HAZ: f32 = 105.0;       // the hazard band along the seam
    const SPINE: f32 = 130.0;     // the spine's half width
    const PISTON: f32 = 270.0;    // where the piston bay ends
    let outer = VW * 0.5 - 330.0; // where the accent wedge begins

    // Bloom from the red lock lights, over everything near them.
    let rl = box_dist(adx, d, 0.0, 34.0, 96.0, 92.0);
    add(&mut sp.glow, RED, (1.0 - rl / 230.0).clamp(0.0, 1.0).powf(2.2) * 0.75);

    if adx < SPINE {
        // The spine: stacked plates, the lock light at the seam.
        sp.h = 1.2;
        if d % 250.0 < 9.0 && d > 100.0 { sp.h = 0.3; }
        // Each spine plate: an engraved outline and a bolt in each corner.
        if d > 110.0 {
            let local = d % 250.0;
            let ring = (adx - 82.0).abs().min((local - 34.0).abs()).min((local - 216.0).abs());
            if ring < 3.0 && adx < 85.0 && local > 31.0 && local < 219.0 { sp.h = 0.85; }
            for by in [20.0, 230.0] {
                let r = ((adx - 104.0).powi(2) + (local - by).powi(2)).sqrt();
                if r < 9.0 { sp.h = 1.35; }
            }
        }
        if rl == 0.0 {
            let core = 1.0 - ((d - 63.0).abs() / 29.0).powf(4.0);
            sp.col = [70.0, 10.0, 12.0];
            add(&mut sp.glow, [255.0, 120.0, 110.0], core.max(0.0) * 1.1);
            sp.h = 0.9;
        } else if rl < 13.0 {
            sp.h = 0.5;
            sp.col = [25.0, 28.0, 32.0];
        }
        if adx > SPINE - 10.0 { sp.h = 0.35; }
        return sp;
    }
    if adx < PISTON {
        // The piston bay: a dark recess with a chrome cylinder in it.
        sp.h = 0.35;
        sp.col = [30.0, 38.0, 44.0];
        let u = (adx - 205.0) / 32.0;
        if u.abs() < 1.0 && d > 20.0 {
            let (h, col) = cyl(u);
            sp.h = h;
            sp.col = col;
        }
        // A thinner hydraulic line beside it.
        let u2 = (adx - 152.0) / 9.0;
        if u2.abs() < 1.0 && d > 20.0 {
            let (h, col) = cyl(u2);
            sp.h = h * 0.9;
            sp.col = col;
        }
        // Collars on the cylinder.
        if (d % 210.0) < 26.0 && d > 30.0 && (adx - 205.0).abs() < 46.0 {
            sp.h = 1.3;
            sp.col = STEEL;
        }
        if adx > PISTON - 10.0 { sp.h = 0.2; }
        return sp;
    }
    if d < HAZ {
        // Hazard stripes along the seam.
        sp.h = 1.05;
        let stripe = (((sx + d) / 64.0).floor() as i32).rem_euclid(2) == 0;
        sp.col = if stripe { [150.0, 26.0, 30.0] } else { [196.0, 196.0, 190.0] };
        if d > HAZ - 9.0 { sp.h = 0.3; sp.col = [24.0, 28.0, 32.0]; }
        return sp;
    }
    // The side: a panel face, a recessed inset with angled cuts, the accent.
    let pd = d - HAZ;
    let accent_edge = outer + pd * 0.55;
    if adx > outer && adx > accent_edge && pd < 520.0 {
        // The accent wedge: its inner edge leans out as it leaves the seam.
        sp.h = 1.15;
        sp.col = ACCENT;
        let e = adx - accent_edge;
        if e < 12.0 { add(&mut sp.glow, STRIP, (1.0 - e / 12.0) * 0.8); }
        if pd > 505.0 { sp.h = 0.3; sp.col = [24.0, 28.0, 32.0]; }
        return sp;
    }
    // The inset: margins in from the spine, the seam and the accent, with a
    // big angled cut at the corner nearest the spine and the seam.
    const R2: f32 = std::f32::consts::SQRT_2;
    let (x0, x1) = (PISTON + 70.0, outer - 60.0);
    let (y0, y1) = (70.0, 780.0);
    let cut = (adx - x0) + (pd - y0) - 300.0; // < 0: inside the cut corner
    let cut2 = (x1 - adx) + (pd - y0) - 150.0;
    let inside = adx > x0 && adx < x1 && pd > y0 && pd < y1 && cut > 0.0 && cut2 > 0.0;
    // Distance in from the inset's edge (approximate, for the rims), and in
    // from its seam-side edges (the bottom and the two cuts).
    let seam_side = (pd - y0).min(cut / R2).min(cut2 / R2);
    let edge = (adx - x0).min(x1 - adx).min(y1 - pd).min(seam_side);

    // The hydraulic lines: two, following the inset's seam-side edges inside
    // it, then out to the piston bay and the accent at the corners.
    const LINES: [f32; 2] = [20.0, 50.0];
    const LR: f32 = 11.0;
    for off in LINES {
        // Inside: along the seam side.
        if inside && (seam_side - off).abs() < LR && edge > off - LR {
            let (h, col) = cyl((seam_side - off) / LR);
            sp.h = h * 0.95;
            sp.col = col;
            if ((adx + pd) % 240.0) < 18.0 { sp.h = 1.2; sp.col = [96.0, 112.0, 120.0]; }
            return sp;
        }
        // Out to the bay: leaves the spine-side cut where it meets the inset's
        // spine-side edge, straight across the face.
        let y_in = y0 + 300.0 + off * R2 - off;
        if adx > PISTON - 10.0 && adx < x0 + off && (pd - y_in).abs() < LR {
            let (h, col) = cyl((pd - y_in) / LR);
            sp.h = h;
            sp.col = col;
            if adx < PISTON + 20.0 { sp.h = 1.3; sp.col = [96.0, 112.0, 120.0]; }
            return sp;
        }
        // And out to the accent.
        let y_out = y0 + 150.0 + off * R2 - off;
        if adx > x1 - off && adx < outer + (y_out) * 0.55 + 4.0 && (pd - y_out).abs() < LR {
            let (h, col) = cyl((pd - y_out) / LR);
            sp.h = h;
            sp.col = col;
            return sp;
        }
    }

    // A caged lamp at each panel's outer top corner, beside its rivet
    // column: laid out on the plate, so it lies with it (and its slant).
    {
        let (lx, ly) = (adx - (x1 + 150.0), pd - (y1 - 10.0));
        if lx.abs() < 40.0 && ly.abs() < 84.0 {
            // Its own light, not the scene's: the same in all four corners
            // (`lamp` skips the shading): the bright glow it always had.
            sp.lamp = true;
            let cage = ly.abs() > 77.0 || lx.abs() > 34.0 || ly.rem_euclid(33.0) < 5.0;
            sp.col = if cage { [120.0, 186.0, 224.0] } else { [175.0, 245.0, 255.0] };
            return sp;
        }
        let g = (1.0 - ((lx / 60.0).powi(2) + (ly / 120.0).powi(2)).sqrt() / 2.6).clamp(0.0, 1.0).powf(2.0) * 0.55;
        add(&mut sp.glow, [70.0, 170.0, 230.0], g);
    }

    if inside {
        // The recess: a bank of heat-sink fins, a row of indicator lights,
        // and a light strip just inside the hydraulic lines.
        sp.h = 0.5;
        sp.col = [32.0, 42.0, 48.0];
        let lx = adx - x0;
        let (fy0, fy1) = (350.0, 700.0);
        let (fx0, fx1) = (90.0, (x1 - x0) - 90.0);
        // The bank's corners cut off at 45°, clear of the light strip where
        // it runs up the recess's angled cuts.
        let clear = seam_side - 104.0;
        if pd > fy0 && pd < fy1 && lx > fx0 && lx < fx1 && clear > 0.0 {
            let f = (lx - fx0) % 26.0;
            sp.h = if f < 9.0 { 0.42 } else { 0.78 };
            sp.col = [70.0, 84.0, 92.0];
            // The fin bank's frame.
            if pd < fy0 + 12.0 || pd > fy1 - 12.0 || lx < fx0 + 12.0 || lx > fx1 - 12.0 || clear < 12.0 {
                sp.h = 0.9;
                sp.col = [86.0, 100.0, 108.0];
            }
        }
        // Indicator lights above the fins, at the far end.
        for k in 0..3 {
            let cx = fx1 - 30.0 - k as f32 * 46.0;
            let r = ((lx - cx).powi(2) + (pd - (fy0 - 26.0)).powi(2)).sqrt();
            if r < 10.0 { sp.col = [20.0, 60.0, 70.0]; add(&mut sp.glow, STRIP, (1.0 - r / 10.0) * 0.9 + 0.2); }
            else if r < 30.0 { add(&mut sp.glow, STRIP, (1.0 - r / 30.0).powf(2.0) * 0.25); }
        }
        // The strip, just inside the hydraulic lines.
        let ps = seam_side - 76.0;
        if ps.abs() < 5.0 { add(&mut sp.glow, STRIP, 0.9); }
        add(&mut sp.glow, STRIP, (1.0 - ps.abs() / 45.0).clamp(0.0, 1.0).powf(2.0) * 0.3);
    } else if adx > x0 - 18.0 && adx < x1 + 18.0 && pd > y0 - 18.0 && pd < y1 + 18.0 && edge > -18.0 {
        // A bevelled frame round the inset.
        sp.h = 1.1;
    } else {
        // The panel face: an engraved second outline round the inset, and a
        // rivet row along it.
        let out = -edge;
        // Not under the pipes' runs across the face.
        if (out - 46.0).abs() < 2.5 { sp.h = 0.75; }
        if (out - 74.0).abs() < 9.0 {
            let along = (adx + pd) % 120.0;
            let r = ((along - 60.0).powi(2) + (out - 74.0).powi(2)).sqrt();
            if r < 8.0 { sp.h = 1.25; }
        }
    }
    sp
}

// ── Signage, after the facility-wall reference ───────────────────────────────
//
// Drawn in SCREEN space (the door as it sits shut), so nothing on the upper
// door is upside down.

/// A triangle pointing up, centred on its centroid at the origin, `s` along
/// a side: the distance in from its nearest edge (negative outside).
fn tri(x: f32, y: f32, s: f32) -> f32 {
    let h = s * 0.866;
    let e_bot = h / 3.0 - y;
    let m = (y + h * 2.0 / 3.0) * (s / (2.0 * h)) - x.abs();
    e_bot.min(m * 0.866)
}

/// Signage at screen point (x, y), in the door's screen space when shut:
/// (colour, emissive) if any: a red caution triangle on the spine, one plate
/// out from the lock lights each way.
fn sign_at(x: f32, y: f32) -> Option<([f32; 3], [f32; 3])> {
    let none = [0.0; 3];
    let white = [232.0, 232.0, 226.0];
    let red = [200.0, 34.0, 36.0];
    // Two triangles on the spine, the second plate out from the lock lights
    // each way. Centred exactly on the plate's engraved square: the plates
    // are 250 apart from the seam and the square's middle is 125 into one,
    // so the third plate's middle is 625 out. And centred by the triangle's
    // box, not its centroid (which sits low in it), so it LOOKS centred.
    {
        let cx = VW * 0.5;
        let seam = seam_y(cx + OVER_X);
        let size = 140.0;
        let box_shift = size * 0.866 / 6.0;
        for cy in [seam - 625.0 + box_shift, seam + 625.0 + box_shift] {
            let t = tri(x - cx, y - cy, size);
            if t > 0.0 {
                if t < 10.0 { return Some((red, none)); }
                let ex = (x - cx).abs() < 7.0 && ((y - cy) > -34.0 && (y - cy) < 9.0 || (y - cy - 24.0).abs() < 7.0);
                return Some((if ex { red } else { white }, none));
            }
        }
    }
    None
}

/// One panel of the door (see `door_spot`): shaded from its own relief, lit
/// by a cool spotlight from above, the lights blooming.
fn panel_img(top: bool) -> Arc<image::RgbaImage> {
    static IMGS: OnceLock<[Arc<image::RgbaImage>; 2]> = OnceLock::new();
    IMGS.get_or_init(|| {
        let make = |top: bool| {
            let k = 0.25; // rasterised at a quarter size, stretched
            let (w, h) = ((panel_w() * k) as u32, (panel_h() * k) as u32);
            Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
                let (fx, fy) = (x as f32 / k, y as f32 / k);
                let at = |fx: f32, fy: f32| -> Option<(Spot, f32, f32)> {
                    let sy = if top { fy - OVER_Y } else { fy + (VH * 0.5 - SLANT * 0.5) };
                    let seam = seam_y(fx);
                    let d = if top { seam - sy } else { sy - seam };
                    if d < -12.0 { return None; }
                    Some((door_spot(fx - OVER_X, d.max(0.0)), d.max(0.0), sy))
                };
                let Some((sp, d, sy)) = at(fx, fy) else { return image::Rgba([0, 0, 0, 0]); };
                // Relief, lit from above: compare with the point just above.
                let nb = at(fx - 3.0, fy - 8.0).map(|(n, _, _)| n.h).unwrap_or(sp.h);
                let slope = (sp.h - nb).clamp(-1.0, 1.0);
                // A cool spotlight from above the middle, falling away.
                let sxs = fx - OVER_X;
                let spot = 0.42 + 0.7 * (-((sxs - VW * 0.5) / 1700.0).powi(2)).exp()
                    * (1.0 - (sy / (VH * 1.4)).clamp(0.0, 1.0) * 0.5);
                // Polished metal: a broad diagonal sheen across the door (the
                // room reflected in it), brushed streaks, and hard highlights
                // on every raised edge.
                let sheen = (-((sxs * 0.5 - sy + 400.0) / 650.0).powi(2)).exp() * 0.45
                    + (-((sxs * 0.5 - sy - 1500.0) / 300.0).powi(2)).exp() * 0.2;
                let brush = (hash2((fx / 2.0).floor() as i32, (fy / 70.0).floor() as i32, 21) - 0.5) * 0.14
                    + (hash2((fx / 9.0).floor() as i32, (fy / 300.0).floor() as i32, 22) - 0.5) * 0.08;
                if sp.lamp {
                    let c = sp.col;
                    return image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]);
                }
                let mut c = [0.0f32; 3];
                for i in 0..3 {
                    let base = sp.col[i] * spot * (0.55 + 0.45 * sp.h.min(1.35)) * (1.0 + sheen + brush);
                    let rim = [190.0, 225.0, 240.0][i];
                    c[i] = if slope > 0.0 { base + rim * slope * 1.1 } else { base * (1.0 + slope * 1.1).max(0.15) };
                }
                // Signage (flat, printed: no sheen).
                {
                    if let Some((col, em)) = sign_at(sxs, sy) {
                        for i in 0..3 { c[i] = col[i] * (0.8 + 0.2 * spot) + em[i]; }
                    }
                }
                // Lights add on top (the bloom).
                for i in 0..3 { c[i] += sp.glow[i]; }
                // Where the doors meet: a thin dark line with a faint red glow.
                let edge = (1.0 - d / 4.0).clamp(0.0, 1.0);
                for i in 0..3 { c[i] = c[i] * (1.0 - edge * 0.7); }
                image::Rgba([c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, 255])
            }))
        };
        [make(true), make(false)]
    })[!top as usize].clone()
}

/// Build the panels' art now (a background thread at app start), so the
/// first transition does not stall while it is generated.
pub fn prewarm() { let _ = (panel_img(true), panel_img(false)); }

fn ensure_panels(c: &mut Canvas) {
    if c.get_game_object(TOP).is_some() { return; }
    for (name, top) in [(TOP, true), (BOT, false)] {
        let size = (panel_w(), panel_h());
        let mut o = GameObject::build(name)
            .position(-OVER_X, if top { -OVER_Y - panel_h() } else { VH })
            .size(size.0, size.1)
            .image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: panel_img(top), color: None })
            // Over everything but the engine's flash.
            .layer(i32::MAX - 1)
            .ignore_zoom()
            .finish();
        o.visible = false;
        // A screen effect: lights must not brighten it.
        o.unlit = true;
        c.add_game_object(name.into(), o);
    }
}

/// Place the panels `amt` closed (0 open, 1 shut).
fn place(c: &mut Canvas, amt: f32) {
    ensure_panels(c);
    let on = amt > 0.0005;
    let travel = panel_h() - OVER_Y + 40.0;
    // Scale the art to the screen HERE. The engine does it in its object
    // update, which does not run while the canvas is paused, and leaving a
    // run from the pause menu closes the doors over a paused game: panels
    // created then (the first transition of a session) kept their image at
    // full virtual size, drawn about twice too big.
    let s = c.base_scale();
    for (name, y) in [
        (TOP, -OVER_Y - travel * (1.0 - amt)),
        (BOT, (VH * 0.5 - SLANT * 0.5) + travel * (1.0 - amt)),
    ] {
        if let Some(o) = c.get_game_object_mut(name) {
            o.visible = on;
            o.position = (-OVER_X, y);
            o.scaled_size.set((o.size.0 * s, o.size.1 * s));
            o.update_image_shape();
        }
    }
}

/// Whether a transition is running (input should wait).
pub fn busy(c: &Canvas) -> bool { c.get_i32("tr_phase") != IDLE }

/// Close the screen, load `scene`, open it.
pub fn go(c: &mut Canvas, scene: &str) {
    go_with(c, scene, false);
}

/// The slower shutter, for leaving a run for the menu.
pub fn go_slow(c: &mut Canvas, scene: &str) {
    go_with(c, scene, true);
}

fn go_with(c: &mut Canvas, scene: &str, slow: bool) {
    register(c);
    if busy(c) { return; }
    c.set_var("tr_slow", slow);
    c.set_var("tr_phase", CLOSING);
    c.set_var("tr_t", 0i32);
    c.set_var("tr_next", Value::Str(scene.to_string()));
    ensure_panels(c);
}

/// Start a scene already closed and open it (for a scene entered some other
/// way that should still open).
pub fn open_now(c: &mut Canvas) {
    register(c);
    c.set_var("tr_phase", OPENING);
    c.set_var("tr_t", 0i32);
    place(c, 1.0);
}

/// Y on the menu: shut the doors and keep them shut; again, open them.
/// (For looking at the door.)
pub fn toggle_hold(c: &mut Canvas) {
    register(c);
    match c.get_i32("tr_phase") {
        IDLE => {
            c.set_var("tr_slow", false);
            c.set_var("tr_phase", CLOSING);
            c.set_var("tr_t", 0i32);
            c.set_var("tr_next", Value::Str(HOLD_SCENE.to_string()));
            ensure_panels(c);
        }
        HELD => {
            c.set_var("tr_phase", OPENING);
            c.set_var("tr_t", 0i32);
        }
        _ => {}
    }
}

/// Register Y for `toggle_hold`, on the menu only. Once, at app start.
pub fn register_hold_key(c: &mut Canvas) {
    if c.get_bool("tr_hold_key_registered") { return; }
    c.set_var("tr_hold_key_registered", true);
    c.on_key_press(|c, key| {
        if *key != Key::Character("y".into()) || !c.is_scene("menu") { return; }
        toggle_hold(c);
    });
    // The same as an event, so the headless driver can press it.
    c.register_custom_event("tr_hold_toggle".into(), |c| { if c.is_scene("menu") { toggle_hold(c); } });
}

fn register(c: &mut Canvas) {
    if c.get_bool("tr_registered") { return; }
    c.set_var("tr_registered", true);
    c.on_update_always(|c| {
        let phase = c.get_i32("tr_phase");
        if phase == IDLE { return; }
        let t = c.get_i32("tr_t") + 1;
        c.set_var("tr_t", t);
        let sp = speed(c);
        let (close, hold_b, hold_a, open) = (CLOSE_TICKS[sp], HOLD_BEFORE[sp], HOLD_AFTER[sp], OPEN_TICKS[sp]);
        match phase {
            HELD => place(c, 1.0),
            CLOSING => {
                let k = (t as f32 / close as f32).min(1.0);
                // Accelerating in, so it lands as a slam.
                place(c, k * k * k);
                if t == close {
                    // The doors slam: a flash off the lit seam, a kick.
                    if let Some(cam) = c.camera_mut() {
                        cam.flash_with(Color(255, 70, 60, 70), 0.2, FlashMode::FadeOut, FlashEase::Sharp, 0.6, 0.0);
                        cam.shake(14.0, 0.18);
                    }
                }
                if t >= close + hold_b {
                    let next = match c.get_var("tr_next") { Some(Value::Str(s)) => s, _ => "menu".into() };
                    if next == HOLD_SCENE {
                        c.set_var("tr_phase", HELD);
                        place(c, 1.0);
                        return;
                    }
                    c.set_var("tr_phase", OPENING);
                    c.set_var("tr_t", 0i32);
                    // Whatever paused the old screen does not carry over.
                    c.resume();
                    c.load_scene(&next);
                    place(c, 1.0);
                }
            }
            _ => {
                if t <= hold_a {
                    place(c, 1.0);
                    return;
                }
                let k = ((t - hold_a) as f32 / open as f32).min(1.0);
                // Unlocking, then sliding apart: a short jolt open (the bolts
                // letting go), a beat, then the doors ease away.
                const JOLT: f32 = 0.16;
                const BEAT: f32 = 0.28;
                let e = if k < JOLT {
                    let j = k / JOLT;
                    0.035 * (j * (2.0 - j))
                } else if k < BEAT {
                    0.035
                } else {
                    let m = (k - BEAT) / (1.0 - BEAT);
                    0.035 + 0.965 * (1.0 - (1.0 - m).powi(3))
                };
                place(c, 1.0 - e);
                if k >= 1.0 {
                    c.set_var("tr_phase", IDLE);
                    place(c, 0.0);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first transition of a session creates the panels, and leaving a
    /// run from the pause menu does that over a PAUSED canvas, where the
    /// engine's object update (which scales images to the window) does not
    /// run. The panels must be scaled anyway, or they draw at full virtual
    /// size: the "first time back to the menu the door is huge" bug.
    #[test]
    fn the_doors_fit_the_window_even_when_first_closed_over_a_paused_game() {
        let (mut ctx, _requests) = Context::new();
        let mut c = Canvas::new(&mut ctx, CanvasMode::Landscape);
        c.resize_for_test((1920.0, 1080.0));
        let s = c.base_scale();
        assert!((s - 0.5).abs() < 1e-4, "window scale {s}");
        c.pause();
        go_slow(&mut c, "menu");
        place(&mut c, 0.5);
        for name in [TOP, BOT] {
            let o = c.get_game_object(name).expect("panel");
            let (w, h) = o.scaled_size.get();
            assert!((w - o.size.0 * s).abs() < 0.5 && (h - o.size.1 * s).abs() < 0.5,
                "{name} drawn at {w}x{h}, want {}x{}", o.size.0 * s, o.size.1 * s);
        }
    }
}
