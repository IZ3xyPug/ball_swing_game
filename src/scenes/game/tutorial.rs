//! The interactive tutorial: a real run, coached step by step.
//!
//! A first-time player starts in a Casual run (no bosses) that cannot be
//! lost, with a panel at the top of the screen giving one instruction at a
//! time. Each step watches what the player actually does — launching, letting
//! go mid-swing, catching nodes, building speed, covering distance — and
//! ticks itself off when it is done; the last steps explain the special
//! nodes. At the end the tutorial is marked done on the profile and the game
//! returns to the menu.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Set before loading the game scene: the next run is the tutorial.
pub const TUTORIAL_PENDING_VAR: &str = "tutorial_pending";
const ACTIVE_VAR: &str = "tutorial_active";
const LAYER: i32 = 140;
const PANEL: (f32, f32) = (1900.0, 300.0);
const PANEL_Y: f32 = 150.0;

struct Step {
    title: &'static str,
    body: &'static str,
}

const STEPS: [Step; 7] = [
    Step { title: "LAUNCH", body: "Hold SPACE (or touch the right half of the screen) to launch." },
    Step { title: "LET GO TO FLY", body: "While you swing forward, let go to fling yourself ahead." },
    Step { title: "CATCH THE NEXT NODE", body: "Hold again near a node to grab it. Chain five grabs." },
    Step { title: "BUILD SPEED", body: "Let go on the upswing to fly faster. Reach top speed 40." },
    Step { title: "GO THE DISTANCE", body: "Keep swinging. Falling to the floor costs a heart (not here)." },
    Step { title: "SPECIAL NODES", body: "Cyan nodes power you up to hurt bosses. Gold nodes shelter you from solar flares." },
    Step { title: "YOU'RE READY", body: "Tutorial complete. Good luck out there!" },
];
const GRABS_NEEDED: u32 = 5;
const SPEED_NEEDED: f32 = 40.0;
const DISTANCE_NEEDED: f32 = 6_000.0;
/// Frames the info steps stay up.
const INFO_FRAMES: u32 = 7 * 60;
const END_FRAMES: u32 = 4 * 60;
/// Frames a finished step shows its tick before the next.
const DONE_FRAMES: u32 = 50;

#[derive(Default)]
struct Track {
    step: usize,
    /// Frames into this step; frames since it was completed (0 = not yet).
    t: u32,
    done_t: u32,
    grabs: u32,
    top_speed: f32,
    start_x: Option<f32>,
    was_hooked: bool,
    shown: Option<(usize, u32, bool)>,
}

fn track() -> &'static Mutex<Track> {
    static T: std::sync::OnceLock<Mutex<Track>> = std::sync::OnceLock::new();
    T.get_or_init(|| Mutex::new(Track::default()))
}

pub fn active(c: &Canvas) -> bool {
    matches!(c.get_var(ACTIVE_VAR), Some(Value::Bool(true)))
}

/// A run is starting: is it the tutorial? Called before the mode is read.
pub fn begin_run(c: &mut Canvas) -> bool {
    let on = matches!(c.get_var(TUTORIAL_PENDING_VAR), Some(Value::Bool(true)))
        || std::env::var("TUTORIAL_RUN").is_ok();
    c.set_var(TUTORIAL_PENDING_VAR, false);
    c.set_var(ACTIVE_VAR, on);
    if on {
        crate::mode::set_mode(c, crate::mode::GameMode::Casual);
        // The tutorial cannot be lost.
        c.set_var("debug_immortal", true);
        c.set_var("tutorial_set_immortal", true);
        *track().lock().unwrap() = Track::default();
    } else if c.get_bool("tutorial_set_immortal") {
        c.set_var("debug_immortal", false);
        c.set_var("tutorial_set_immortal", false);
    }
    on
}

/// The panel's objects, for the scene builder (hidden).
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut v = Vec::new();
    for (id, layer) in [("tut_panel", 0), ("tut_step", 1), ("tut_title", 1), ("tut_body", 1), ("tut_progress", 1), ("tut_skip", 1)] {
        let mut o = GameObject::build(id).size(PANEL.0, 60.0).position(-9000.0, -9000.0).tag("hud").build(ctx);
        o.ignore_zoom = true;
        o.unlit = true;
        o.visible = false;
        o.layer = LAYER + layer;
        v.push((id.to_string(), o));
    }
    v
}

fn text(c: &mut Canvas, id: &str, t: &str, px: f32, col: Color, y: f32) {
    let Some(font) = super::helpers::ui_font() else { return };
    let s = c.virtual_scale();
    let x = (VW - PANEL.0) * 0.5;
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = (PANEL.0, px * 1.4);
        o.position = (x, y);
        o.set_drawable(Box::new(crate::objects::ui_text_spec(t, &font, px * s, col, (PANEL.0 - 80.0) * s)));
        o.visible = true;
    }
}

fn hide(c: &mut Canvas) {
    for id in ["tut_panel", "tut_step", "tut_title", "tut_body", "tut_progress", "tut_skip"] {
        if let Some(o) = c.get_game_object_mut(id) { o.visible = false; }
    }
}

fn draw(c: &mut Canvas, step: usize, progress: &str, done: bool) {
    let s = &STEPS[step];
    let accent = if done { [130, 235, 160] } else { [120, 200, 255] };
    if let Some(o) = c.get_game_object_mut("tut_panel") {
        o.size = PANEL;
        o.position = ((VW - PANEL.0) * 0.5, PANEL_Y);
        o.set_image(crate::menu_look::glass(PANEL.0, PANEL.1, accent, false));
        o.visible = true;
    }
    let n = STEPS.len() - 1;
    let label = if step < n { format!("TUTORIAL   \u{00b7}   STEP {} OF {}", step + 1, n) } else { "TUTORIAL".to_string() };
    text(c, "tut_step", &label, 30.0, Color(accent[0], accent[1], accent[2], 230), PANEL_Y + 22.0);
    let title = if done { format!("\u{2713}  {}", s.title) } else { s.title.to_string() };
    text(c, "tut_title", &title, 56.0, if done { Color(150, 245, 180, 255) } else { Color(255, 240, 200, 255) }, PANEL_Y + 64.0);
    text(c, "tut_body", s.body, 36.0, Color(225, 230, 245, 240), PANEL_Y + 150.0);
    text(c, "tut_progress", progress, 32.0, Color(190, 215, 255, 230), PANEL_Y + 214.0);
    text(c, "tut_skip", "ESC  \u{2192}  MENU to skip the tutorial", 26.0, Color(170, 170, 200, 170), PANEL_Y + PANEL.1 + 14.0);
}

/// Every game frame of a tutorial run.
pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !active(c) { return; }
    let (hooked, px, speed, dead) = {
        let s = st.lock().unwrap();
        (s.hooked, s.px, (s.vx * s.vx + s.vy * s.vy).sqrt(), s.dead)
    };
    if dead { return; }
    let launched = !c.get_bool("start_prompt_active");
    let mut finish = false;
    let (step, progress, done) = {
        let mut t = track().lock().unwrap();
        t.t += 1;
        let grabbed = hooked && !t.was_hooked;
        let released = !hooked && t.was_hooked;
        t.was_hooked = hooked;
        if launched { t.top_speed = t.top_speed.max(speed); }
        // Is this step done?
        let complete = match t.step {
            0 => launched,
            1 => released && speed >= 12.0,
            2 => { if grabbed { t.grabs += 1; } t.grabs >= GRABS_NEEDED }
            3 => t.top_speed >= SPEED_NEEDED,
            4 => {
                let x0 = *t.start_x.get_or_insert(px);
                px - x0 >= DISTANCE_NEEDED
            }
            5 => t.t >= INFO_FRAMES,
            _ => t.t >= END_FRAMES,
        };
        if complete && t.done_t == 0 { t.done_t = 1; }
        if t.done_t > 0 { t.done_t += 1; }
        // Next step once the tick has been seen (info steps move on at once).
        let pause = if t.step >= 5 { 1 } else { DONE_FRAMES };
        if t.done_t > pause {
            if t.step + 1 >= STEPS.len() {
                finish = true;
            } else {
                t.step += 1;
                t.t = 0;
                t.done_t = 0;
                if t.step == 3 { t.top_speed = 0.0; }
            }
        }
        let progress = match t.step {
            2 => format!("{} / {} grabs", t.grabs.min(GRABS_NEEDED), GRABS_NEEDED),
            3 => format!("top speed  {:.0} / {:.0}", t.top_speed.min(SPEED_NEEDED), SPEED_NEEDED),
            4 => format!("{} / {} px", (px - t.start_x.unwrap_or(px)).max(0.0) as i32, DISTANCE_NEEDED as i32),
            _ => String::new(),
        };
        (t.step, progress, t.done_t > 0)
    };
    if finish {
        complete(c);
        return;
    }
    // Redraw when anything shown changed (progress text is re-made cheaply
    // only when its number moves).
    let key = (step, progress.len() as u32 * 1000 + progress.bytes().map(|b| b as u32).sum::<u32>(), done);
    let changed = track().lock().unwrap().shown != Some(key);
    if changed {
        track().lock().unwrap().shown = Some(key);
        draw(c, step, &progress, done);
        if done {
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(130, 235, 160, 70), 0.25, FlashMode::Pulse, FlashEase::Sharp, 0.5, 0.0);
            }
        }
    }
}

/// The tutorial is finished: mark it, and back to the menu.
fn complete(c: &mut Canvas) {
    {
        let g = crate::profile::profile();
        g.lock().unwrap().tutorial_done = true;
    }
    crate::profile::save_profile();
    crate::achievements::on_tutorial_done();
    end(c);
    crate::transition::go(c, "menu");
}

/// Leave tutorial mode (finished, or the game scene is left).
pub fn end(c: &mut Canvas) {
    if active(c) {
        c.set_var(ACTIVE_VAR, false);
        if c.get_bool("tutorial_set_immortal") {
            c.set_var("debug_immortal", false);
            c.set_var("tutorial_set_immortal", false);
        }
    }
    hide(c);
}
