//! The MYSTIC trail's twin strands, as on its shop card: two ribbons twisting
//! round the cat's path, crossing back and forth like a double helix.
//!
//! An emitter can only follow an object, so each strand follows an invisible
//! guide moved every frame to the cat's position plus a swing across its
//! direction of travel (opposite for the two), and draws a ribbon behind it.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::state::*;

pub const GUIDES: [&str; 2] = ["mystic_guide_a", "mystic_guide_b"];
pub const STRANDS: [&str; 2] = ["player_trail_mystic_a", "player_trail_mystic_b"];
const VAR: &str = "mystic_trail_on";
/// How far each strand swings from the path, and how fast it twists.
const AMP: f32 = 34.0;
const TWIST: f32 = 0.32;

/// The guides, for the scene builder (invisible, no physics).
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    GUIDES.iter().map(|id| {
        let mut o = GameObject::new_rect(ctx, id.to_string().into(), None::<Image>, (2.0, 2.0), (-9000.0, -9000.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        o.visible = false;
        o.gravity = 0.0;
        (id.to_string(), o)
    }).collect()
}

/// Turn the strands on (MYSTIC) or off (any other trail).
pub fn set(c: &mut Canvas, on: bool) {
    for s in STRANDS { c.remove_emitter(s); }
    c.set_var(VAR, on);
    if !on { return; }
    for (strand, guide) in STRANDS.iter().zip(GUIDES) {
        c.add_emitter(strand_emitter(strand));
        c.attach_emitter_to(strand, guide);
    }
}

/// One strand's ribbon (also the shop preview's).
pub fn strand_emitter(name: &str) -> Emitter {
    EmitterBuilder::new(name)
        .rate(260.0).lifetime(0.5).velocity(0.0, 0.0).spread(4.0, 4.0)
        .size(15.0).size_end(3.0).shape(ParticleShape::Circle)
        .color(255, 175, 245, 235).color_end(130, 60, 240, 0)
        .interpolate_position(true).render_layer(3).gravity_scale(0.0)
        .collision(CollisionResponse::None).build()
}

/// Place two guides weaving round a point moving at `v`, `t` frames in.
pub fn weave(c: &mut Canvas, guides: [&str; 2], t: i32, p: (f32, f32), v: (f32, f32)) {
    let sp = (v.0 * v.0 + v.1 * v.1).sqrt();
    // Across the direction of travel (straight up and down when still).
    let (nx, ny) = if sp > 0.5 { (-v.1 / sp, v.0 / sp) } else { (0.0, 1.0) };
    let w = (t as f32 * TWIST).sin() * AMP;
    for (i, g) in guides.iter().enumerate() {
        let k = if i == 0 { w } else { -w };
        if let Some(o) = c.get_game_object_mut(g) {
            o.position = (p.0 + nx * k - 1.0, p.1 + ny * k - 1.0);
            o.momentum = (0.0, 0.0);
        }
    }
}

/// Every game frame: the guides weave round the cat.
pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !matches!(c.get_var(VAR), Some(Value::Bool(true))) { return; }
    let (px, py, vx, vy) = { let s = st.lock().unwrap(); (s.px, s.py, s.vx, s.vy) };
    let t = c.get_i32("mystic_trail_t") + 1;
    c.set_var("mystic_trail_t", t);
    weave(c, GUIDES, t, (px, py), (vx, vy));
}
