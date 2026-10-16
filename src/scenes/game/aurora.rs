//! The northern lights over the planet's atmosphere.
//!
//! One sprite in the sky (`aurora`), drawn by the engine's `Effect::Aurora`
//! shader: curtains that fold and ripple, rays sliding along them, patches
//! brightening and fading, coloured by height. It lives in the sky image's
//! own rows like the nebula (see `nebula.rs`), so it zooms with the sky, sits
//! behind the planets and the world, and turns over with the sky when
//! gravity flips. Like everything that far away it slides sideways only as
//! the player travels forward, and slowly.
//!
//! A display of under a minute starts every five minutes of play
//! (`AURORA_EVERY_SECS`), the whole of it fading in and out together. U
//! starts one at once (and, during one, fades it away).

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

const ID: &str = "aurora";
/// Where the quad sits, in sky-image rows: from the planet's limb in the
/// earth photo (row ~1,600) up about 0.7 screen heights. The shader floats
/// the arc's lower edge a little above its bottom (the limb), with clear
/// atmosphere between, as in the reference backdrop.
/// (Moved down 60 rows: it sat a touch high over the limb.)
pub const AURORA_TOP_ROW: f32 = 1150.0;
pub const AURORA_BOTTOM_ROW: f32 = 1720.0;
/// Width, as a multiple of the sky image's (centred, room for its faded ends).
pub const AURORA_WIDTH: f32 = 2.0;
/// Sideways drift, in sky-image px per px of forward travel: nearer than
/// the nebula (it is in the atmosphere), still very slow.
pub const AURORA_PARALLAX: f32 = 0.02;
/// How strong it is at full display (0..1).
pub const AURORA_INTENSITY: f32 = 0.85;
/// A display starts every this many seconds of play...
pub const AURORA_EVERY_SECS: f32 = 300.0;
/// ...and lasts this long (seconds, a range).
pub const AURORA_SHOW_SECS: (f32, f32) = (35.0, 55.0);
/// Seconds to fade in or out.
pub const AURORA_FADE_SECS: f32 = 10.0;
/// The key that starts a display now (or, during one, fades it away).
pub const AURORA_KEY: &str = "u";
/// The shader's clock wraps here, in seconds, to keep its noise precise.
const CLOCK_WRAP: f32 = 3000.0;

#[derive(Default, Clone)]
pub struct AuroraTick {
    /// Seconds on the aurora's own clock.
    time: f32,
    /// How far it has slid left, in sky-image px.
    scroll: f32,
    prev_px: Option<f32>,
    /// The display schedule: showing or not, seconds left in this display,
    /// seconds until the next one, and how far faded in (0..1).
    showing: bool,
    left: f32,
    next_in: f32,
    fade: f32,
    rng: u32,
    started: bool,
    key_was_down: bool,
}

impl AuroraTick {
    fn rand(&mut self) -> f32 {
        // xorshift: the display lengths only need to not look regular.
        let mut x = self.rng.max(1);
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.rng = x;
        (x % 10_000) as f32 / 10_000.0
    }
    fn span(&mut self, r: (f32, f32)) -> f32 { r.0 + (r.1 - r.0) * self.rand() }

    /// Start a display now; the next one is a full interval after it.
    fn start(&mut self) {
        self.showing = true;
        self.left = self.span(AURORA_SHOW_SECS);
        self.next_in = AURORA_EVERY_SECS;
    }

    /// Advance the schedule a frame; the display's strength, 0..1. `key` is
    /// the aurora key's press this frame.
    fn advance(&mut self, key: bool) -> f32 {
        let dt = 1.0 / 60.0;
        if !self.started {
            self.started = true;
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos()).unwrap_or(1);
            self.rng = seed | 1;
            self.next_in = AURORA_EVERY_SECS;
        }
        if key {
            if self.showing { self.showing = false; self.left = 0.0; } else { self.start(); }
        }
        if self.showing {
            self.left -= dt;
            if self.left <= 0.0 { self.showing = false; }
        } else if self.fade <= 0.0 {
            // The clock to the next display runs while the sky is clear.
            self.next_in -= dt;
            if self.next_in <= 0.0 { self.start(); }
        }
        let step = dt / AURORA_FADE_SECS;
        self.fade = if self.showing { (self.fade + step).min(1.0) } else { (self.fade - step).max(0.0) };
        // Eased, so the whole display swells in and dies away together.
        self.fade * self.fade * (3.0 - 2.0 * self.fade)
    }
}

/// Placed right after the nebula: over the far sky, under everything else.
pub fn aurora_object(ctx: &mut Context) -> (String, GameObject) {
    let mut obj = GameObject::new_rect(ctx, ID.to_string(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: crate::images::solid(0, 0, 0, 0).into(), color: None }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    obj.ignore_zoom = true;
    obj.visible = false;
    (ID.to_string(), obj)
}

/// After `tick_background` has placed the sky (`bg`) for this frame.
pub fn tick_aurora(c: &mut Canvas, st: &Arc<Mutex<State>>, t: &mut AuroraTick) {
    let (px, flipped, no_sky, boss) = {
        let s = st.lock().unwrap();
        (s.px, s.gravity_dir < 0.0, s.in_space_mode || s.space_launch_active, s.boss_active)
    };
    // Headless film mode: `AURORA_ON=1` forces a display, for looking at it.
    let key_down = c.key(AURORA_KEY);
    let pressed = key_down && !t.key_was_down;
    t.key_was_down = key_down;
    // The displays are the ICE WORLD's: only over it does the clock run
    // (twice as fast, so they come every couple of minutes). The key still
    // calls one up anywhere, for looking at it.
    let ice = crate::sky::PLANETS[crate::sky::planet_of(c)].aurora;
    let idle = !t.showing && t.fade <= 0.0;
    if ice && idle { t.next_in -= 1.0 / 60.0; }
    let strength = if std::env::var("AURORA_ON").is_ok() { 1.0 }
        else if !ice && idle && !pressed { 0.0 }
        else { t.advance(pressed) };
    let show = !no_sky && !boss && strength > 0.0 && crate::graphics::aurora_enabled();
    c.set_var("aurora_up", show && strength > 0.5);
    let Some(((x, y), (w, h))) = c.get_game_object("bg").map(|o| (o.position, o.size)) else { return };
    if !show {
        c.clear_effect(ID);
        if let Some(obj) = c.get_game_object_mut(ID) { obj.visible = false; }
        return;
    }
    t.time = (t.time + 1.0 / 60.0) % CLOCK_WRAP;
    // Drift with the CAMERA, not the player: the sky it lies on follows the
    // camera, which smooths the swing, so following the player's own motion
    // made it shudder against the sky with every lurch of the swing. Both
    // ways, as parallax does; a jump of 2,000 px or more is a warp or respawn.
    let _ = px;
    let cam_x = c.camera().map_or(0.0, |cam| cam.position.0);
    let dx = t.prev_px.map_or(0.0, |p| cam_x - p);
    t.prev_px = Some(cam_x);
    let forward = if dx.abs() > 2_000.0 { 0.0 } else { dx };
    let layer_w = VW * AURORA_WIDTH;
    t.scroll += forward * AURORA_PARALLAX;

    let (sx, sy) = (w / VW, h / VH);
    let (top, bottom) = (AURORA_TOP_ROW, AURORA_BOTTOM_ROW);
    // Flipped, the sky image is upside down, so the rows mirror about it.
    let ry = if flipped { y + (VH - bottom) * sy } else { y + top * sy };
    let (qw, qh) = (layer_w * sx, (bottom - top) * sy);
    let qx = x + (VW - layer_w) / 2.0 * sx;
    if let Some(obj) = c.get_game_object_mut(ID) {
        // Exact, not rounded: the sky under it moves by fractions of a
        // pixel, and rounding the quad's place and size separately made it
        // shuffle a pixel against the sky and rescale the effect each frame.
        obj.position = (qx, ry);
        obj.size = (qw, qh);
        obj.rotation = if flipped { 180.0 } else { 0.0 };
        obj.visible = true;
    }
    c.attach_effect(ID,
        Effect::Aurora { intensity: AURORA_INTENSITY * strength, aspect: (bottom - top) / layer_w, time: t.time, scroll: t.scroll / layer_w },
        EffectColor::WHITE, (qw, qh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(t: &mut AuroraTick, secs: f32, key_at: Option<u32>) -> Vec<f32> {
        (0..(secs * 60.0) as u32).map(|f| t.advance(key_at == Some(f))).collect()
    }

    #[test]
    fn a_display_comes_every_five_minutes_and_fades_as_one() {
        let mut t = AuroraTick::default();
        // Nothing for the first five minutes.
        let first = run(&mut t, AURORA_EVERY_SECS - 1.0, None);
        assert!(first.iter().all(|&s| s == 0.0), "no display before five minutes");
        // Then one: it swells to full, holds, and fades away again.
        let shown = run(&mut t, 2.0 + AURORA_SHOW_SECS.1 + AURORA_FADE_SECS + 1.0, None);
        assert!(shown.iter().any(|&s| s >= 0.999), "the display reaches full strength");
        assert_eq!(*shown.last().unwrap(), 0.0, "and fades out completely");
        // Rising then falling only: one fade in, one fade out, no flicker.
        let peak = shown.iter().position(|&s| s >= 0.999).unwrap();
        assert!(shown[..peak].windows(2).all(|w| w[1] >= w[0]));
        let last_full = shown.iter().rposition(|&s| s >= 0.999).unwrap();
        assert!(shown[last_full..].windows(2).all(|w| w[1] <= w[0]));
    }

    #[test]
    fn the_key_starts_a_display_and_ends_one() {
        let mut t = AuroraTick::default();
        let on = run(&mut t, AURORA_FADE_SECS + 1.0, Some(10));
        assert!(*on.last().unwrap() >= 0.999, "U starts a display at once");
        let off = run(&mut t, AURORA_FADE_SECS + 1.0, Some(0));
        assert_eq!(*off.last().unwrap(), 0.0, "U again fades it away");
    }
}
