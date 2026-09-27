use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
#[allow(unused_imports)]
use super::*;

// ── The Gravity Weaver ──────────────────────────────────────────────────────
//
// A loom that weaves gravity. Fifth fight of the run — the first of the back
// half — so it escalates rather than teaches: everything the Colossus and the
// Conductor established (telegraph, lane, marker, window) is here, plus one
// thing nothing before it does: the WORLD TURNS OVER.
//
// Five parts. Four SPINDLES orbit the LOOM. A spindle's attack is the
// SHUTTLE: it draws a lane through the player, then throws a gravity thread
// along it across the whole arena. Dodge the thread; the spindle is open while
// it winds the thread back in. The loom's attack is the INVERSION: on a clock,
// the spindles draw in, a gravity lens opens over the loom, and gravity flips —
// the player falls UP until the next flip. Every live part is exposed for a
// moment after a flip, so surviving one well-placed is the big punish.
//
// Spindles come in pairs (0,1 live; 2,3 shielded until they are gone), and
// the loom is shielded until every spindle is dead. Once it is, the loom
// throws shuttles itself and flips faster. 4 x 2 + 14 = 22 hits: two a
// spindle (crack, then kill) since playtesting found five far too many.
//
// The flip is REAL: `State::gravity_dir` is inverted, which is what the
// space rifts do. The one rule that could not survive that unchanged was the
// fall check (see `State::fall_ceiling_y`): inverted, "fell off the top"
// used to mean any y above -150, which is every point in the arena.

/// Index of the loom in `boss_parts`; the spindles are 0..WEAVER_SPINDLES.
pub(crate) const WEAVER_LOOM_IDX: usize = WEAVER_SPINDLES;

/// Ticks a part rests between shuttles, staggered per spindle so no two
/// throw in lockstep.
fn weaver_idle_len(i: usize) -> u32 {
    if i == WEAVER_LOOM_IDX {
        return WEAVER_LOOM_IDLE_TICKS;
    }
    WEAVER_IDLE_TICKS + (i as u32 * 23) % 61
}

fn weaver_part_size(i: usize) -> f32 {
    if i == WEAVER_LOOM_IDX { WEAVER_LOOM_SIZE } else { WEAVER_SPINDLE_SIZE }
}

fn weaver_part_hit_r(i: usize) -> f32 {
    weaver_part_size(i) * 0.42
}

/// Where spindle `i` sits relative to the loom. `draw_in` (0..1) is how far
/// the inversion telegraph has pulled the ring in toward the loom.
pub(crate) fn weaver_spindle_offset(i: usize, orbit: f32, draw_in: f32) -> (f32, f32) {
    let a = orbit + i as f32 * std::f32::consts::FRAC_PI_2;
    let k = 1.0 - WEAVER_DRAW_IN * draw_in;
    (a.cos() * WEAVER_ORBIT_RX * k, a.sin() * WEAVER_ORBIT_RY * k)
}

/// Is the player within reach of a thread laid from `start` along `dir`
/// (unit) for `len`? Pure, so the hit test is testable without a canvas.
pub(crate) fn thread_hits(
    start: (f32, f32), dir: (f32, f32), len: f32, p: (f32, f32), half_w: f32,
) -> bool {
    let end = (start.0 + dir.0 * len, start.1 + dir.1 * len);
    point_segment_dist(p, start, end) < half_w
}

/// Which side of the thread the player is on, as a unit normal pointing away
/// from the line — the direction a strike throws them.
fn thread_normal(start: (f32, f32), dir: (f32, f32), p: (f32, f32)) -> (f32, f32) {
    let n = (-dir.1, dir.0);
    let side = (p.0 - start.0) * n.0 + (p.1 - start.1) * n.1;
    if side >= 0.0 { n } else { (-n.0, -n.1) }
}

/// Hide every Weaver piece. Called on victory (the fight stops being ticked
/// the moment the boss dies) and on every fight start.
pub(crate) fn hide_weaver(c: &mut Canvas) {
    for i in 0..=WEAVER_LOOM_IDX {
        let part = format!("weaver_part_{i}");
        c.clear_effect(&part);
        c.clear_overlay_effect(&part);
        if let Some(o) = c.get_game_object_mut(&part) {
            o.visible = false;
            o.position = (-9000.0, -9000.0);
            o.rotation = 0.0;
        }
        let thread = format!("weaver_thread_{i}");
        c.clear_effect(&thread);
        if let Some(o) = c.get_game_object_mut(&thread) {
            o.visible = false;
            o.position = (-9000.0, -9000.0);
        }
    }
    c.clear_effect("weaver_lens");
    if let Some(o) = c.get_game_object_mut("weaver_lens") {
        o.visible = false;
        o.position = (-9000.0, -9000.0);
    }
}

/// Put gravity the right way up again. Every exit from the fight — victory,
/// a fall, a death — goes through here, or the next thing the player does
/// happens upside down.
pub(crate) fn weaver_restore_gravity(c: &mut Canvas, s: &mut State) {
    s.weaver_inverted = false;
    s.gravity_dir = 1.0;
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = obj.gravity.abs();
    }
}

/// Turn the world over. Shared by the loom's clock and the gauntlet.
fn weaver_flip(s: &mut State, core_phase: bool, check: bool) {
    s.weaver_inverted = !s.weaver_inverted;
    s.gravity_dir = if s.weaver_inverted { -1.0 } else { 1.0 };
    s.weaver_flips += 1;
    s.weaver_flip_ticks = if core_phase { WEAVER_FLIP_INTERVAL_CORE } else { WEAVER_FLIP_INTERVAL };
    s.weaver_flip_open = WEAVER_FLIP_OPEN_TICKS;
    if check {
        eprintln!("weaver-check: flip {} (inverted={})", s.weaver_flips, s.weaver_inverted);
    }
}

/// Put gauntlet shot `shot` (0-based) on its post's spindle: a short
/// wind-up aimed through the player, ignoring the pattern cooldown.
fn weaver_gauntlet_arm(s: &mut State, loom_x: f32, shot: usize, player: (f32, f32), vel: (f32, f32)) {
    let i = s.weaver_gauntlet_parts[shot % 2];
    let side = if shot % 2 == 0 { -1.0 } else { 1.0 };
    let p = &mut s.boss_parts[i];
    p.state = PartState::Telegraph;
    p.state_ticks = WEAVER_TELEGRAPH_TICKS.saturating_sub(WEAVER_GAUNTLET_TELEGRAPH);
    p.post_attack = false;
    p.beam_hit_done = false;
    // Aim from the post through the player.
    let post = (loom_x + side * WEAVER_GAUNTLET_POST_DX, s.weaver_gauntlet_y);
    let aim = (player.0 + vel.0 * WEAVER_AIM_LEAD, player.1 + vel.1 * WEAVER_AIM_LEAD);
    let dx = aim.0 - post.0;
    let dy = aim.1 - post.1;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    s.boss_parts[i].target = (dx / d, dy / d);
}

pub(crate) fn tick_gravity_weaver(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let s = st.lock().unwrap();
        if !s.boss_active || s.dead || s.boss_stasis_active { return; }
    }
    let check = std::env::var("WEAVER_CHECK").is_ok();
    // WEAVER_REACH_CHECK: a buffed stand-in flies at a player's speed to the
    // nearest open part and logs how far into each window its first hit
    // lands, or that the window closed first. It measures whether the
    // windows are REACHABLE; WEAVER_CHECK (which pins the player on the
    // part) only proves that hits register.
    let reach = std::env::var("WEAVER_REACH_CHECK").is_ok();

    // ── Appearance (once) ──────────────────────────────────────────────────
    let spawned = { st.lock().unwrap().boss_spawned };
    if !spawned {
        let name = {
            let mut s = st.lock().unwrap();
            s.boss_spawned = true;
            // A fresh clock every time the fight (re)starts — after a fall
            // the loom must not flip the world the frame the player lands.
            s.weaver_flip_ticks = WEAVER_FLIP_INTERVAL;
            s.weaver_flip_open = 0;
            s.weaver_orbit = 0.0;
            s.boss_pattern_cooldown = WEAVER_PATTERN_COOLDOWN;
            s.boss_entry_ticks = 0;
            // A gauntlet interrupted by a fall does not resume; it re-arms.
            s.weaver_gauntlet = 0;
            s.weaver_gauntlet_ticks = 0;
            for p in s.boss_parts.iter_mut() {
                p.state = PartState::Idle;
                p.state_ticks = 0;
                p.post_attack = false;
                p.beam_hit_done = false;
            }
            s.boss_kind.name()
        };
        hide_weaver(c);
        let cx = arena_center_x(c);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, BOSS_Y_CENTER - BOSS_SIZE * 0.5);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(WEAVER_SRGB.0, WEAVER_SRGB.1, WEAVER_SRGB.2, 255), 1000.0 * sc,
                )));
            }
        }
    }

    // ── Clocks ─────────────────────────────────────────────────────────────
    let (px, py, pvx, pvy, buffed) = {
        let mut s = st.lock().unwrap();
        s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
        if s.boss_pattern_cooldown > 0 { s.boss_pattern_cooldown -= 1; }
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
        if s.boss_contact_cooldown > 0 { s.boss_contact_cooldown -= 1; }
        if s.weaver_flip_open > 0 { s.weaver_flip_open -= 1; }
        s.weaver_orbit += WEAVER_ORBIT_RATE;
        s.boss_phase += 0.006;
        if check || reach {
            // The checks park the player inside the fight; they are proving
            // the boss's paths, not the player's survival, so hearts stay up.
            s.hearts = s.hearts.max(3);
        }
        (s.px, s.py, s.vx, s.vy, s.buff_active())
    };
    if check || reach {
        c.set_var("debug_force_buff", true);
    }

    // The loom drifts slowly; the spindles ride its orbit.
    let (bcx, bcy) = {
        let phase = st.lock().unwrap().boss_phase;
        let cx = arena_center_x(c) + (phase * 0.7).sin() * 1500.0;
        let cy = BOSS_Y_CENTER + (phase * 0.5 + 0.5).sin() * 700.0;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, cy - BOSS_SIZE * 0.5);
        }
        (cx, cy)
    };

    // ── Phase gating ───────────────────────────────────────────────────────
    // Pairs of spindles, then the loom. Written here rather than through the
    // shared "unshield part i once every part before it is dead" loop, which
    // would have opened the spindles one at a time.
    let core_phase = {
        let mut s = st.lock().unwrap();
        let dead = |s: &State, i: usize| !s.boss_parts[i].alive;
        let first_pair_dead = dead(&s, 0) && dead(&s, 1);
        let all_spindles_dead = (0..WEAVER_SPINDLES).all(|i| dead(&s, i));
        for i in 0..WEAVER_SPINDLES {
            let open = i < 2 || first_pair_dead;
            let p = &mut s.boss_parts[i];
            if p.alive && p.shielded && open {
                p.shielded = false;
                if check { eprintln!("weaver-check: spindle {i} unshielded"); }
            }
        }
        let loom = &mut s.boss_parts[WEAVER_LOOM_IDX];
        if loom.alive && loom.shielded && all_spindles_dead {
            loom.shielded = false;
            if check { eprintln!("weaver-check: loom exposed"); }
        }
        // The gauntlet re-arms at each phase change: once when the second
        // pair opens, once when a single spindle is left.
        let live: Vec<usize> = (0..WEAVER_SPINDLES).filter(|&i| s.boss_parts[i].alive).collect();
        let want = if first_pair_dead && live.len() >= 2 && s.weaver_gauntlet_done & 1 == 0 {
            Some((1u8, [live[0], live[1]]))
        } else if first_pair_dead && live.len() == 1 && s.weaver_gauntlet_done & 2 == 0 {
            Some((2u8, [live[0], live[0]]))
        } else {
            None
        };
        if let Some((bit, parts)) = want {
            if s.weaver_gauntlet == 0 {
                s.weaver_gauntlet_done |= bit;
                s.weaver_gauntlet = 1;
                s.weaver_gauntlet_ticks = 0;
                s.weaver_gauntlet_parts = parts;
                s.weaver_gauntlet_y = s.py.clamp(BOSS_ARENA_NODE_Y_BOT + 400.0, BOSS_ARENA_NODE_Y_TOP - 400.0);
                s.weaver_gauntlet_fired = false;
                if check { eprintln!("weaver-check: gauntlet {bit} begins with spindles {parts:?}"); }
            }
        }
        all_spindles_dead
    };

    // ── The inversion ──────────────────────────────────────────────────────
    // A count-in, then the world turns over. The telegraph is the loom's lens
    // opening and the spindles drawing in; nothing else may begin an attack
    // while it runs, so the flip is the only thing there is to read.
    let (mut flip_now, telegraph, draw_in) = {
        let mut s = st.lock().unwrap();
        let mut flip_now = false;
        // The loom's clock holds while a gauntlet runs: its shots flip the
        // world themselves, and a count-in on top would say two things.
        let gauntlet = s.weaver_gauntlet > 0;
        if s.weaver_flip_ticks > 0 && !gauntlet {
            s.weaver_flip_ticks -= 1;
        }
        let tel = !gauntlet && s.weaver_flip_ticks < WEAVER_FLIP_TELEGRAPH;
        let draw_in = if tel {
            1.0 - s.weaver_flip_ticks as f32 / WEAVER_FLIP_TELEGRAPH as f32
        } else {
            0.0
        };
        if s.weaver_flip_ticks == 0 && !gauntlet {
            weaver_flip(&mut s, core_phase, check);
            flip_now = true;
        }
        (flip_now, tel, draw_in)
    };

    // ── The gauntlet ──────────────────────────────────────────────────────
    // Two posts, one each side of the loom at the height the player was at
    // when it began. Shots alternate posts: A, B, A. Each shot's thread is
    // the flip's trigger, so the fall the flip causes is INTO the thread
    // unless the player has read the lane and moved.
    let (gauntlet_step, gauntlet_align) = {
        let mut s = st.lock().unwrap();
        let step = s.weaver_gauntlet;
        let mut align = 0.0;
        if step == 1 {
            s.weaver_gauntlet_ticks += 1;
            align = (s.weaver_gauntlet_ticks as f32 / WEAVER_GAUNTLET_ALIGN_TICKS as f32).min(1.0);
            if s.weaver_gauntlet_ticks >= WEAVER_GAUNTLET_ALIGN_TICKS {
                s.weaver_gauntlet = 2;
                s.weaver_gauntlet_ticks = 0;
                s.weaver_gauntlet_fired = false;
                weaver_gauntlet_arm(&mut s, bcx, 0, (px, py), (pvx, pvy));
                if check { eprintln!("weaver-check: gauntlet shot 1 armed"); }
            }
        } else if step >= 2 && step < 2 + WEAVER_GAUNTLET_SHOTS {
            align = 1.0;
            let shot = (step - 2) as usize;
            // A lone spindle crosses to the other post before each shot after
            // the first; the clock drives its position (see the frame loop).
            let lone = s.weaver_gauntlet_parts[0] == s.weaver_gauntlet_parts[1];
            if lone && shot > 0 && s.weaver_gauntlet_ticks < WEAVER_GAUNTLET_TRANSIT_TICKS {
                s.weaver_gauntlet_ticks += 1;
                // Arrived: aim NOW, from the post, at where the player is —
                // not where they were before it crossed the arena.
                if s.weaver_gauntlet_ticks == WEAVER_GAUNTLET_TRANSIT_TICKS {
                    weaver_gauntlet_arm(&mut s, bcx, shot, (px, py), (pvx, pvy));
                    if check { eprintln!("weaver-check: lone spindle crossed to post {}", shot % 2); }
                }
            }
            let shooter = s.weaver_gauntlet_parts[shot % 2];
            let state = s.boss_parts[shooter].state;
            let alive = s.boss_parts[shooter].alive;
            if !alive {
                // Killed mid-gauntlet: nothing left to fire it.
                s.weaver_gauntlet = 2 + WEAVER_GAUNTLET_SHOTS;
                s.weaver_gauntlet_ticks = 0;
            } else if state == PartState::Attack && !s.weaver_gauntlet_fired {
                s.weaver_gauntlet_fired = true;
                weaver_flip(&mut s, core_phase, check);
                flip_now = true;
            } else if s.weaver_gauntlet_fired && state != PartState::Attack {
                // Thread back in: next post fires, or the gauntlet is over.
                s.weaver_gauntlet += 1;
                s.weaver_gauntlet_ticks = 0;
                s.weaver_gauntlet_fired = false;
                if s.weaver_gauntlet < 2 + WEAVER_GAUNTLET_SHOTS {
                    let next = (s.weaver_gauntlet - 2) as usize;
                    weaver_gauntlet_arm(&mut s, bcx, next, (px, py), (pvx, pvy));
                    if check { eprintln!("weaver-check: gauntlet shot {} armed", next + 1); }
                } else if check {
                    eprintln!("weaver-check: gauntlet over, ring resumes");
                }
            }
        } else if step == 2 + WEAVER_GAUNTLET_SHOTS {
            s.weaver_gauntlet_ticks += 1;
            align = 1.0 - (s.weaver_gauntlet_ticks as f32 / WEAVER_GAUNTLET_RELEASE_TICKS as f32).min(1.0);
            if s.weaver_gauntlet_ticks >= WEAVER_GAUNTLET_RELEASE_TICKS {
                s.weaver_gauntlet = 0;
                s.weaver_gauntlet_ticks = 0;
            }
        }
        (step, align)
    };
    if flip_now {
        // The engine's gravity is re-synced from `gravity_dir` every frame,
        // but the flip should be FELT this frame, not the next.
        let gdir = st.lock().unwrap().gravity_dir;
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * gdir;
        }
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(
                Color(WEAVER_SRGB.0, WEAVER_SRGB.1, WEAVER_SRGB.2, 150),
                0.35, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0,
            );
        }
    }
    // The lens over the loom: opens through the telegraph, snaps shut on the
    // flip, and shows faintly all the time so the loom always reads as the
    // thing bending space.
    {
        let d = WEAVER_LOOM_SIZE * WEAVER_LENS_SCALE;
        let strength = if telegraph { 0.25 + 0.75 * draw_in } else { 0.12 };
        if let Some(o) = c.get_game_object_mut("weaver_lens") {
            o.size = (d, d);
            o.position = (bcx - d * 0.5, bcy - d * 0.5);
            o.visible = true;
        }
        let (r, g, b) = WEAVER_LENS_SRGB;
        c.attach_effect("weaver_lens", Effect::GravityLens { strength },
                        EffectColor::srgb8(r, g, b), (d, d));
    }

    // ── Per-part FSM ───────────────────────────────────────────────────────
    #[derive(Clone, Copy)]
    struct Frame {
        alive: bool,
        shielded: bool,
        weak_open: bool,
        pos: (f32, f32),
        /// Thread direction (unit) and how much of the lane the telegraph has
        /// filled; `thread_live` while the thread is out.
        dir: (f32, f32),
        lane: Option<f32>,
        thread_live: bool,
        /// Reeling the thread back in: 0 just after the throw, 1 fully in.
        retract: Option<f32>,
        damaged: bool,
        strike: Option<(f32, f32)>,
        /// How close an open window is to shutting (0..1), for the marker's
        /// closing flicker.
        closing: Option<f32>,
    }
    let frames: Vec<Frame> = {
        let mut s = st.lock().unwrap();
        let orbit = s.weaver_orbit;
        let cooldown_ok = s.boss_pattern_cooldown == 0;
        let flip_open = s.weaver_flip_open > 0;
        let flip_closing = 1.0 - s.weaver_flip_open as f32 / WEAVER_FLIP_OPEN_TICKS as f32;
        let mut frames = Vec::with_capacity(s.boss_parts.len());
        let mut began = false;
        let g_parts = s.weaver_gauntlet_parts;
        let g_y = s.weaver_gauntlet_y;
        let lone = g_parts[0] == g_parts[1];
        // Which post the lone spindle is at (or heading for), where it came
        // from, and how far across it is. Past the last shot it stays at the
        // last shot's post, not the next one, while it drifts home.
        let g_shot = (gauntlet_step.clamp(2, 1 + WEAVER_GAUNTLET_SHOTS) - 2) as usize;
        let transit = if gauntlet_step >= 2 && gauntlet_step < 2 + WEAVER_GAUNTLET_SHOTS && g_shot > 0 {
            (s.weaver_gauntlet_ticks as f32 / WEAVER_GAUNTLET_TRANSIT_TICKS as f32).min(1.0)
        } else {
            1.0
        };
        let in_transit = lone && transit < 1.0;
        for i in 0..s.boss_parts.len() {
            let pos = if i == WEAVER_LOOM_IDX {
                (bcx, bcy)
            } else {
                let o = weaver_spindle_offset(i, orbit, draw_in);
                let ring = (bcx + o.0, bcy + o.1);
                // On a post during the gauntlet: left for slot 0, right for
                // slot 1 (a lone spindle takes whichever post its shot is
                // from), blended from the ring while lining up and back.
                let slot = if gauntlet_step > 0 && lone && g_parts[0] == i {
                    Some(g_shot % 2)
                } else if gauntlet_step > 0 && g_parts[0] == i {
                    Some(0)
                } else if gauntlet_step > 0 && g_parts[1] == i {
                    Some(1)
                } else {
                    None
                };
                let post_of = |k: usize| {
                    let side = if k == 0 { -1.0 } else { 1.0 };
                    (bcx + side * WEAVER_GAUNTLET_POST_DX, g_y)
                };
                let placed = match slot {
                    // The lone spindle crossing the arena: eased, so it
                    // visibly launches and settles.
                    Some(k) if lone && transit < 1.0 => {
                        let e = transit * transit * (3.0 - 2.0 * transit);
                        lerp2(post_of((k + 1) % 2), post_of(k), e)
                    }
                    Some(k) => lerp2(ring, post_of(k), gauntlet_align),
                    None => ring,
                };
                // An open spindle HOLDS where it threw from, then eases back.
                let q = &s.boss_parts[i];
                if q.alive && q.state == PartState::Recover {
                    q.path_start
                } else if q.alive && q.state == PartState::Idle && q.post_attack
                    && q.state_ticks < WEAVER_REJOIN_TICKS
                {
                    let t = q.state_ticks as f32 / WEAVER_REJOIN_TICKS as f32;
                    lerp2(q.path_start, placed, t * t * (3.0 - 2.0 * t))
                } else {
                    placed
                }
            };
            let p = &mut s.boss_parts[i];
            let damaged = p.hp * 2 <= p.max_hp;
            if !p.alive {
                frames.push(Frame { alive: false, shielded: false, weak_open: false, pos,
                    dir: (1.0, 0.0), lane: None, thread_live: false, retract: None,
                    damaged, strike: None, closing: None });
                continue;
            }
            if p.shielded {
                p.state = PartState::Idle;
                p.state_ticks = 0;
                p.post_attack = false;
                frames.push(Frame { alive: true, shielded: true, weak_open: false, pos,
                    dir: (1.0, 0.0), lane: None, thread_live: false, retract: None,
                    damaged, strike: None, closing: None });
                continue;
            }
            let mut strike = None;
            match p.state {
                PartState::Idle => {
                    p.state_ticks += 1;
                    // Not during a post-flip window either: a part that opened
                    // on the flip must STAY open for it, not start a wind-up
                    // a few frames in (which flashed "hit me" and shut again).
                    if cooldown_ok && !telegraph && !began && gauntlet_step == 0 && !flip_open
                        && p.state_ticks >= weaver_idle_len(i)
                    {
                        p.state = PartState::Telegraph;
                        p.state_ticks = 0;
                        p.post_attack = false;
                        p.beam_hit_done = false;
                        // Aim through (a little ahead of) the player, and lock
                        // it: the lane is a promise.
                        let aim = (px + pvx * WEAVER_AIM_LEAD, py + pvy * WEAVER_AIM_LEAD);
                        let dx = aim.0 - pos.0;
                        let dy = aim.1 - pos.1;
                        let d = (dx * dx + dy * dy).sqrt().max(1.0);
                        p.target = (dx / d, dy / d);
                        began = true;
                    }
                }
                PartState::Telegraph => {
                    // A lone spindle's wind-up waits until it has crossed to
                    // its post: the lane is aimed from there.
                    let frozen = in_transit && g_parts[0] == i;
                    if !frozen {
                        p.state_ticks += 1;
                    }
                    if p.state_ticks >= WEAVER_TELEGRAPH_TICKS {
                        p.state = PartState::Attack;
                        p.state_ticks = 0;
                    }
                }
                PartState::Attack => {
                    p.state_ticks += 1;
                    if !p.beam_hit_done
                        && thread_hits(pos, p.target, WEAVER_THREAD_LEN, (px, py),
                                       WEAVER_THREAD_HIT_HALF + PLAYER_R)
                    {
                        p.beam_hit_done = true;
                        strike = Some(thread_normal(pos, p.target, (px, py)));
                    }
                    if p.state_ticks >= WEAVER_SHUTTLE_TICKS {
                        p.state = PartState::Recover;
                        p.state_ticks = 0;
                        p.post_attack = true;
                        // Where it holds for the window.
                        p.path_start = pos;
                    }
                }
                PartState::Recover => {
                    p.state_ticks += 1;
                    if p.state_ticks >= WEAVER_RECOVER_TICKS {
                        p.state = PartState::Idle;
                        p.state_ticks = 0;
                    }
                }
            }
            // Open while winding the thread back in, and — for every live
            // part at once — for a moment after a flip. The post-flip window
            // needs no `post_attack`: the flip IS the attack it follows.
            // A lone spindle mid-gauntlet is re-armed the moment its thread is
            // out, so its "window" would last a frame: a vulnerable flash that
            // shuts at once. It stays closed until its last shot.
            let gauntlet_busy = lone && g_parts[0] == i
                && gauntlet_step >= 2 && gauntlet_step < 1 + WEAVER_GAUNTLET_SHOTS;
            let weak_open = !gauntlet_busy
                && (matches!(p.state, PartState::Recover)
                    || (flip_open && matches!(p.state, PartState::Idle)));
            // Window bookkeeping for the reach check: `weakpoint_window`
            // counts ticks open, `attack_timer` hits landed in this window.
            if p.weakpoint_open && !weak_open {
                if reach && p.attack_timer == 0 {
                    eprintln!("reach: part {i} window CLOSED unhit after {} ticks", p.weakpoint_window);
                }
                p.weakpoint_window = 0;
                p.attack_timer = 0;
            }
            if weak_open {
                p.weakpoint_window += 1;
            }
            p.weakpoint_open = weak_open;
            // The lane clears just before the thread flies (the last
            // `WEAVER_TELEGRAPH_CLEAR` ticks), so the thread lands on a clean
            // frame instead of on top of its own warning.
            let lane_up = p.state == PartState::Telegraph
                && p.state_ticks + WEAVER_TELEGRAPH_CLEAR < WEAVER_TELEGRAPH_TICKS
                && !(in_transit && g_parts[0] == i);
            let closing = if p.state == PartState::Recover {
                Some(p.state_ticks as f32 / WEAVER_RECOVER_TICKS as f32)
            } else if weak_open {
                Some(flip_closing)
            } else {
                None
            };
            let retract = (p.state == PartState::Recover && p.state_ticks < WEAVER_RETRACT_TICKS)
                .then(|| p.state_ticks as f32 / WEAVER_RETRACT_TICKS as f32);
            frames.push(Frame {
                alive: true, shielded: false, weak_open, pos, dir: p.target,
                lane: if lane_up {
                    Some(p.state_ticks as f32 / WEAVER_TELEGRAPH_TICKS as f32)
                } else { None },
                thread_live: p.state == PartState::Attack,
                retract,
                damaged, strike, closing,
            });
        }
        if began { s.boss_pattern_cooldown = WEAVER_PATTERN_COOLDOWN; }
        frames
    };

    if reach {
        let target = frames
            .iter()
            .filter(|f| f.alive && f.weak_open)
            .map(|f| f.pos)
            .min_by(|a, b| {
                let da = (a.0 - px).powi(2) + (a.1 - py).powi(2);
                let db = (b.0 - px).powi(2) + (b.1 - py).powi(2);
                da.total_cmp(&db)
            })
            .unwrap_or((bcx, bcy + 1800.0));
        let (dx, dy) = (target.0 - px, target.1 - py);
        let d = (dx * dx + dy * dy).sqrt();
        if d > 1.0 {
            let step = d.min(WEAVER_REACH_SPEED);
            let np = (px + dx / d * step, py + dy / d * step);
            {
                let mut s = st.lock().unwrap();
                s.px = np.0;
                s.py = np.1;
                s.vx = 0.0;
                s.vy = 0.0;
                s.hooked = false;
            }
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.position = (np.0 - PLAYER_R, np.1 - PLAYER_R);
                obj.momentum = (0.0, 0.0);
            }
        }
    }

    // ── Visuals, threads, hits ─────────────────────────────────────────────
    let mut any_alive = false;
    let (tr, tg, tb) = WEAVER_SRGB;
    for (i, f) in frames.iter().enumerate() {
        let part = format!("weaver_part_{i}");
        let thread = format!("weaver_thread_{i}");
        let size = weaver_part_size(i);
        if !f.alive {
            c.clear_effect(&part);
            crate::scenes::game::fx::clear_windup(c, &part);
            c.clear_effect(&thread);
            if let Some(o) = c.get_game_object_mut(&part) { o.visible = false; }
            if let Some(o) = c.get_game_object_mut(&thread) { o.visible = false; }
            continue;
        }
        any_alive = true;

        // The part itself. A spindle turns to lie along its thread while it
        // is throwing, so the bobbin reads as the thing the thread comes off.
        if let Some(o) = c.get_game_object_mut(&part) {
            o.size = (size, size);
            o.position = (f.pos.0 - size * 0.5, f.pos.1 - size * 0.5);
            o.visible = true;
            if i != WEAVER_LOOM_IDX {
                let want = if f.lane.is_some() || f.thread_live {
                    f.dir.1.atan2(f.dir.0).to_degrees() - 90.0
                } else {
                    0.0
                };
                let mut delta = (want - o.rotation).rem_euclid(360.0);
                if delta > 180.0 { delta -= 360.0; }
                o.rotation += delta * 0.18;
            }
            // Damaged art after half its HP is gone: a state the sprite
            // carries itself, like the Devourer's generators.
            let want_art: &'static [u8] = match (i == WEAVER_LOOM_IDX, f.damaged) {
                (true, true) => ASSET_PL_WEAVER_LOOM_DAMAGED,
                (true, false) => ASSET_PL_WEAVER_LOOM,
                (false, true) => ASSET_PL_WEAVER_SPINDLE_DAMAGED,
                (false, false) => ASSET_PL_WEAVER_SPINDLE,
            };
            let showing_damaged = o.animated_sprite.is_none();
            if f.damaged && !showing_damaged {
                if let Some(img) = crate::scenes::game::helpers::pl_image_cached(want_art, size) {
                    o.animated_sprite = None;
                    o.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (size, size), 0.0),
                        image: img, color: None,
                    });
                    if check { eprintln!("weaver-check: part {i} shows its damaged art"); }
                }
            }
        }

        // State, ALWAYS shown: the phase shield, the open burn, or — live but
        // closed — a lighter shell. A part with no marker read as hittable,
        // so a spindle winding up looked open when it was not.
        {
            let (rgb, k, mode) = if f.shielded {
                (WEAVER_MARKER_SHIELDED_RGB, 0.85, MarkerMode::Shielded)
            } else if f.weak_open {
                (WEAVER_MARKER_VULNERABLE_RGB, 1.0, MarkerMode::Vulnerable)
            } else {
                (WEAVER_MARKER_SHIELDED_RGB, WEAVER_CLOSED_SHELL, MarkerMode::Shielded)
            };
            crate::scenes::game::fx::attach_state_marker_timed(
                c, &part, (size, size), rgb, k, mode,
                if mode == MarkerMode::Vulnerable { f.closing } else { None });
            // The telegraph, OVER the state: the flip's count-in on every
            // part, and each spindle's own wind-up.
            let windup = if telegraph {
                let beats = (draw_in * 3.0 * std::f32::consts::PI).sin().abs();
                Some(0.55 + 0.45 * beats)
            } else if f.lane.is_some() {
                Some(0.95)
            } else {
                None
            };
            match windup {
                Some(k) => crate::scenes::game::fx::attach_windup(
                    c, &part, (size, size), WEAVER_MARKER_WINDUP_RGB, k),
                None => crate::scenes::game::fx::clear_windup(c, &part),
            }
        }

        // The lane while winding up, the thread while it is out, and the
        // thread winding back in (shortening) while the part is open.
        // Reeling in, the strip SHORTENS toward the spindle and is turned end
        // for end, so the tether's energy flows back INTO it: the thread
        // returning, not a line erasing itself from the spindle outward.
        let strip = if f.lane.is_some() || f.thread_live || f.retract.is_some() {
            let len = match f.retract {
                Some(t) => WEAVER_THREAD_LEN * (1.0 - t * t * (3.0 - 2.0 * t)).max(0.02),
                None => WEAVER_THREAD_LEN,
            };
            let w = WEAVER_THREAD_W;
            let mid = (f.pos.0 + f.dir.0 * len * 0.5, f.pos.1 + f.dir.1 * len * 0.5);
            let flip = if f.retract.is_some() { 180.0 } else { 0.0 };
            if let Some(o) = c.get_game_object_mut(&thread) {
                o.size = (len, w);
                o.rotation = f.dir.1.atan2(f.dir.0).to_degrees() + flip;
                o.position = (mid.0 - len * 0.5, mid.1 - w * 0.5);
                o.visible = true;
            }
            Some((len, w))
        } else {
            None
        };
        match strip {
            Some((len, w)) if f.thread_live => {
                // HOT: the weave says "this will hurt" — the same line
                // winding back in, below, is calm, and safe to cross.
                c.attach_effect(&thread, Effect::EnergyTether { intensity: 1.0, snap: None, hot: true },
                                EffectColor::srgb8(tr, tg, tb), (len, w));
            }
            Some((len, w)) if f.retract.is_some() => {
                // Calm (no weave): reeling in, it is safe to cross.
                c.attach_effect(&thread, Effect::EnergyTether { intensity: 0.6, snap: None, hot: false },
                                EffectColor::srgb8(tr, tg, tb), (len, w));
            }
            Some((len, w)) => {
                c.attach_effect(&thread,
                    Effect::StrikeLane { intensity: 1.0, progress: f.lane.unwrap_or(0.0) },
                    crate::scenes::game::fx::lin(WEAVER_MARKER_WINDUP_RGB), (len, w));
            }
            None => {
                c.clear_effect(&thread);
                if let Some(o) = c.get_game_object_mut(&thread) { o.visible = false; }
            }
        }

        // A thread strike: thrown off the line, off the rope, and a heart —
        // or, buffed, one absorption spent.
        if let Some(n) = f.strike {
            let kick = (n.0 * WEAVER_THREAD_KICK, n.1 * WEAVER_THREAD_KICK);
            {
                let mut s = st.lock().unwrap();
                s.vx = kick.0;
                s.vy = kick.1;
                s.hooked = false;
                s.active_hook = String::new();
            }
            c.run(Action::Hide { target: Target::name("rope") });
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = kick;
            }
            c.set_var("boss_knockback_ticks", Value::I32(18));
            if buffed {
                let mut s = st.lock().unwrap();
                s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
                if s.buff_absorbs == 0 { s.retire_buff(); }
            } else {
                let dead = { st.lock().unwrap().dead };
                if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
            }
        }

        // SOLID: the player cannot pass through a part, and touching one
        // throws them back off it. What the touch MEANS depends on the part:
        // open and buffed, a hit (felt: flash, shake, hit-stop, rebound);
        // closed, the contact penalty; open but unbuffed, just the bounce.
        if check && f.weak_open && buffed {
            // WEAVER_CHECK: park the player on the first open part, so every
            // hit / damaged-art / gating / flip path runs in a real fight
            // loop. The auto-player never lands a buffed hit.
            let mut s = st.lock().unwrap();
            if s.hitstop_ticks == 0 {
                s.px = f.pos.0;
                s.py = f.pos.1;
            }
        }
        if let Some(normal) = crate::scenes::game::boss::common::bounce_off_part(
            c, st, f.pos, weaver_part_hit_r(i))
        {
            if f.weak_open && buffed {
                let mut landed = false;
                {
                    let mut s = st.lock().unwrap();
                    if s.boss_part_invuln_ticks == 0 {
                        let p = &mut s.boss_parts[i];
                        if p.alive {
                            p.attack_timer += 1;
                            if reach && p.attack_timer == 1 {
                                eprintln!("reach: part {i} first hit {} ticks into its window", p.weakpoint_window);
                            }
                            p.hp -= 1;
                            if check {
                                eprintln!("weaver-check: hit part {i} (hp {} -> {})", p.hp + 1, p.hp);
                            }
                            if p.hp <= 0 {
                                p.alive = false;
                                if check { eprintln!("weaver-check: part {i} destroyed"); }
                            }
                            landed = true;
                        }
                        if landed {
                            s.boss_part_invuln_ticks = WEAVER_PART_INVULN_TICKS;
                            s.buff_hit_flash = 20;
                        }
                    }
                }
                if landed {
                    let at = (f.pos.0 + normal.0 * weaver_part_hit_r(i), f.pos.1 + normal.1 * weaver_part_hit_r(i));
                    crate::scenes::game::boss::common::land_hit(
                        c, st, &part, at, size * 0.6, WEAVER_MARKER_VULNERABLE_RGB, normal);
                }
            } else if !f.weak_open {
                // Touching a part you cannot hit costs a heart, once per
                // cooldown; buffed, the bounce alone (it already took you
                // off the rope).
                let penalise = {
                    let mut s = st.lock().unwrap();
                    let fire = s.boss_contact_cooldown == 0 && !s.dead;
                    if fire {
                        s.boss_contact_cooldown = 45;
                    }
                    fire && !buffed
                };
                if penalise {
                    crate::scenes::game::hearts::lose_heart(c, st);
                }
            }
        }
    }

    // HUD total.
    {
        let mut s = st.lock().unwrap();
        s.boss_hp = boss_total_hp(&s);
        s.boss_hp_max = s.boss_hp.max(1);
    }

    // ── Win ────────────────────────────────────────────────────────────────
    if !any_alive {
        if check { eprintln!("weaver-check: loom destroyed, fight over"); }
        {
            let mut s = st.lock().unwrap();
            weaver_restore_gravity(c, &mut s);
        }
        hide_weaver(c);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        finish_boss(c, st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_reaches_across_the_arena_and_no_further() {
        let start = (0.0, 0.0);
        let dir = (1.0, 0.0);
        assert!(thread_hits(start, dir, 1000.0, (500.0, 10.0), 40.0));
        assert!(!thread_hits(start, dir, 1000.0, (500.0, 80.0), 40.0), "beside the thread");
        assert!(!thread_hits(start, dir, 1000.0, (1200.0, 0.0), 40.0), "past its end");
        assert!(!thread_hits(start, dir, 1000.0, (-200.0, 0.0), 40.0), "behind the spindle");
    }

    #[test]
    fn a_strike_throws_the_player_off_the_side_they_are_on() {
        let n = thread_normal((0.0, 0.0), (1.0, 0.0), (300.0, 25.0));
        assert!(n.1 > 0.0, "below the thread (y down) is thrown further down: {n:?}");
        let n = thread_normal((0.0, 0.0), (1.0, 0.0), (300.0, -25.0));
        assert!(n.1 < 0.0, "{n:?}");
    }

    #[test]
    fn the_ring_draws_in_during_the_telegraph() {
        let far = weaver_spindle_offset(0, 0.0, 0.0);
        let near = weaver_spindle_offset(0, 0.0, 1.0);
        assert!(near.0.abs() < far.0.abs() * 0.7, "{far:?} -> {near:?}");
        // Four spindles a quarter turn apart.
        let a = weaver_spindle_offset(0, 0.0, 0.0);
        let b = weaver_spindle_offset(2, 0.0, 0.0);
        assert!((a.0 + b.0).abs() < 1e-3 && (a.1 + b.1).abs() < 1e-3, "opposite: {a:?} {b:?}");
    }
}
