use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use crate::scenes::game::bootstrap::hook_asteroid_anim_for_spawn;
use crate::scenes::game::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;
#[allow(unused_imports)]
use super::*;

/// Serpent chain offset: the head leads the body and each segment trails behind
/// it on a travelling sine wave. The amplitude grows toward the tail and the wave
/// phase lags per segment, so the whole body undulates end-to-end like a serpent
/// rather than sitting in a fixed diagonal. Segment 0 is nearest the head.
// ── Body follow ──────────────────────────────────────────────────────────────

/// Record where the head is now, and how far it has travelled to get there.
///
/// The body reads this history rather than a formula. A shared sine wave makes
/// every segment bob on the same curve at a phase offset, which reads as a
/// wobbling chain; sampling the leader's actual path makes each segment arrive
/// exactly where the head was, which reads as a body following a head.
pub(crate) fn serpent_push_trail(
    trail: &mut Vec<(f32, f32, f32)>,
    arc: &mut f32,
    head: (f32, f32),
) {
    let step = match trail.first() {
        Some((px, py, _)) => ((head.0 - px).powi(2) + (head.1 - py).powi(2)).sqrt(),
        None => 0.0,
    };
    // Skip a sample the head has barely moved from: identical points make the
    // arc-length search below degenerate without adding any shape.
    if !trail.is_empty() && step < 1.0 {
        return;
    }
    *arc += step;
    trail.insert(0, (head.0, head.1, *arc));
    trail.truncate(SERPENT_TRAIL_LEN);
}

/// The point `behind` px back along the head's path.
///
/// Interpolates between the two samples that bracket the distance, so segment
/// spacing stays even no matter how fast the head is moving — at speed the
/// samples are far apart, and snapping to the nearest one would make the body
/// visibly concertina.
pub(crate) fn serpent_trail_point(
    trail: &[(f32, f32, f32)],
    arc: f32,
    behind: f32,
) -> Option<((f32, f32), f32)> {
    let target = arc - behind;
    if trail.is_empty() {
        return None;
    }
    let mut prev: Option<&(f32, f32, f32)> = None;
    for sample in trail.iter() {
        if sample.2 <= target {
            let (x, y, arc) = *sample;
            let (px, py, heading) = match prev {
                Some(&(px, py, parc)) => {
                    let span = (parc - arc).max(0.0001);
                    let t = ((target - arc) / span).clamp(0.0, 1.0);
                    (x + (px - x) * t, y + (py - y) * t, (py - y).atan2(px - x))
                }
                // Newer than the newest sample: the head itself. Its heading
                // comes from the two most recent samples — the direction it is
                // travelling. Returning 0 here left the head drawn facing right
                // no matter which way the serpent was going, which is why it
                // looked pasted on rather than leading the body.
                None => {
                    let dir = trail
                        .get(1)
                        .map(|&(bx, by, _)| (y - by).atan2(x - bx))
                        .unwrap_or(0.0);
                    (x, y, dir)
                }
            };
            return Some(((px, py), heading.to_degrees()));
        }
        prev = Some(sample);
    }
    // The body is longer than the trail (start of a fight): fall back to the
    // oldest sample so the tail trails off the end rather than snapping home.
    trail.last().map(|&(x, y, _)| ((x, y), 0.0))
}

/// How far back along the body a part sits, in px of head-trail arc length.
/// `chain` is 0 for the head, 1..N for segments, N+1 for the tail.
pub(crate) fn serpent_chain_distance(chain: usize) -> f32 {
    chain as f32 * SERPENT_SEGMENT_SPACING
}

/// How many links the body currently has: one per LIVING part.
///
/// Everything that walks the chain — spacing, the shield band, the sweep pivot —
/// measures against this rather than against `SERPENT_SEGMENTS`, so the body
/// genuinely shortens as it is dismantled instead of keeping empty slots.
pub(crate) fn serpent_chain_span(parts: &[BossPart]) -> usize {
    parts.iter().filter(|p| p.alive).count().max(1)
}

/// Where a part sits in the CHAIN, which is not its index in `boss_parts`.
///
/// `boss_parts` is ordered by DESTRUCTION dependency — segments, then tail, then
/// head — because the shared loop unshields part `i` once everything before it
/// is dead. The chain runs head, segments, tail. Conflating the two puts the
/// head at the back of its own body.
///
/// A segment's link is its RANK AMONG THE LIVING, not its index. Ranking by
/// index left a permanent hole in the body wherever a segment had died: the
/// survivors kept their original spacing and the serpent flew around with a
/// visible bite out of its middle. Ranking by survivor closes the gap, and the
/// body gets shorter with every kill — which is the read the fight wants
/// anyway, since a shorter serpent is a faster one.
pub(crate) fn serpent_chain_index(parts: &[BossPart], part_index: usize, part_id: &str) -> usize {
    match part_id {
        "head" => 0,
        // Immediately behind the last living segment, wherever that now is.
        "tail" => 1 + parts.iter().filter(|p| p.id == "seg" && p.alive).count(),
        _ => {
            let ahead = parts
                .iter()
                .take(part_index)
                .filter(|p| p.id == "seg" && p.alive)
                .count();
            1 + ahead
        }
    }
}

/// Half-width of the energised band, in links, for a body of `span` links.
///
/// DERIVED from the open fraction rather than authored, because the window the
/// player actually gets is what matters and the width is only the mechanism.
///
/// A segment is open when its cyclic distance from the band's centre satisfies
/// `seam >= SERPENT_OPEN_AT`, i.e. `d >= half * (1 + OPEN_AT)`, and `d` runs
/// from 0 to `span / 2`. Setting the open fraction `f` and solving:
///
/// ```text
///   f = 1 - half * (1 + OPEN_AT) / (span / 2)
///   half = (1 - f) * span / (2 * (1 + OPEN_AT))
/// ```
///
/// Scaling with `span` is the point: a width in LINKS is a larger share of a
/// shorter body, so a fixed width shrank the open window as segments died. The
/// player got less able to damage the serpent the more of it they had killed.
fn serpent_band_half(span: usize) -> f32 {
    (1.0 - SERPENT_OPEN_FRACTION) * span as f32 / (2.0 * (1.0 + SERPENT_OPEN_AT))
}

/// Is this segment inside the energised band this frame?
///
/// The band is a moving run of segments, not a per-segment blink — see
/// `SERPENT_SHIELD_BAND`. Distance is measured cyclically along the body so the
/// band wraps from tail back to head without a seam.
pub(crate) fn serpent_shielded_now(chain: usize, band: f32, span: usize) -> bool {
    let s = span as f32;
    let d = (chain as f32 - band).rem_euclid(s);
    let d = d.min(s - d);
    d < serpent_band_half(span)
}

/// How lit a segment's seam is, 0 armoured to 1 fully exposed. The inverse of
/// the shield, eased so the plating visibly opens rather than popping.
pub(crate) fn serpent_seam(chain: usize, band: f32, span: usize) -> f32 {
    let s = span as f32;
    let d = (chain as f32 - band).rem_euclid(s);
    let d = d.min(s - d);
    let half = serpent_band_half(span);
    ((d - half) / half).clamp(0.0, 1.0)
}

pub(crate) fn serpent_part_offset(i: f32, phase: f32) -> (f32, f32) {
    // Trail horizontally behind the head with a slight downward cascade.
    let x = -(i + 1.0) * 220.0;
    let base_y = i * 70.0 - 240.0;
    // Wave travels down the chain: tail amplitude is larger and lags further.
    let amp = 70.0 + i * 18.0;
    let wave = (phase - i * 0.9).sin();
    // A little coiling on x too, but mostly a lateral (y) undulation.
    let y = base_y + wave * amp;
    let x_wave = (phase - i * 0.9).cos() * (10.0 + i * 4.0);
    (x + x_wave, y)
}

/// The Serpent: a multi-part boss whose body IS the level. Eight segments trail
/// the head in a chain; a buffed hit destroys a segment, which shortens + speeds
/// the body. The head is invulnerable until every segment is gone, then it
/// detaches and is the only target. Touching a live segment or the head costs a
/// heart.
// ── Act selection ────────────────────────────────────────────────────────────

/// Which acts the Serpent can perform right now.
///
/// Availability is decided by WHICH PARTS SURVIVE, not by a phase counter. That
/// keeps the fight's structure and its anatomy the same statement: the tail
/// launch exists because there is a tail, the spine lash exists because the
/// segments between head and tail are gone, and the gambit is what is left when
/// only a head remains. A phase number would let those drift apart.
pub(crate) fn serpent_acts(parts: &[BossPart]) -> Vec<SerpentAct> {
    let alive = |id: &str| parts.iter().any(|p| p.id == id && p.alive);
    let segments = parts.iter().filter(|p| p.id == "seg" && p.alive).count();
    let mut acts = Vec::new();
    if segments > 0 {
        acts.push(SerpentAct::Coil);
        acts.push(SerpentAct::RiftStrikes);
    }
    if alive("tail") && segments > 0 {
        acts.push(SerpentAct::TailLaunch);
        acts.push(SerpentAct::TailSweep);
    }
    if segments == 0 && alive("tail") && alive("head") {
        acts.push(SerpentAct::SpineLash);
    }
    if segments == 0 && !alive("tail") && alive("head") {
        acts.push(SerpentAct::WormholeGambit);
    }
    acts
}

/// How long the current act runs.
fn serpent_act_len(act: SerpentAct) -> u32 {
    match act {
        SerpentAct::Prowl => SERPENT_ATTACK_GAP,
        SerpentAct::Coil => SERPENT_COIL_TICKS,
        SerpentAct::TailSweep => SERPENT_SWEEP_TICKS,
        // Generous, because the burrow is driven by a state machine and its
        // real length depends on how far the head has to swim to each portal.
        // This is a CEILING that ends the act if something stalls, not the
        // schedule — the sequence normally finishes on its own.
        SerpentAct::RiftStrikes => {
            let swallow = (serpent_body_arc() / SERPENT_RIFT_SWALLOW_SPEED) as u32;
            let swim = (SERPENT_RIFT_AHEAD / SERPENT_HEAD_SPEED) as u32;
            SERPENT_TELEGRAPH_TICKS
                + (SERPENT_RIFT_COUNT + 1)
                    * (swim + swallow + SERPENT_RIFT_HIDDEN + SERPENT_RIFT_SURFACE)
        }
        SerpentAct::TailLaunch => SERPENT_TAIL_LAUNCH_TICKS,
        SerpentAct::SpineLash => SERPENT_LASH_TICKS,
        SerpentAct::WormholeGambit => SERPENT_TELEGRAPH_TICKS + 180,
    }
}

/// Where the head wants to be this frame, given what it is doing.
pub(crate) fn serpent_goal_for(act: SerpentAct, ticks: u32, player: (f32, f32), centre: (f32, f32),
                    vel: (f32, f32)) -> (f32, f32) {
    match act {
        // Ring the arena, then close. The radius shrinks over the act so the
        // player can see the noose tightening and pick a gap before it does.
        // A spiral, not a ring: the radius shrinks continuously while the head
        // laps, so the player watches the loop tighten around the spot they
        // were standing in and has to leave through a gap before it closes.
        //
        // `centre` here is the COIL CENTRE — the player's position captured
        // when the act began, not the arena's middle. Centring it on the arena
        // put the coil wherever the player was not, which is why it never read
        // as a trap; and centring it on their LIVE position would mean it never
        // closes at all.
        SerpentAct::Coil => {
            let t = (ticks as f32 / SERPENT_COIL_TICKS as f32).clamp(0.0, 1.0);
            let r = SERPENT_COIL_RADIUS + (SERPENT_COIL_CLOSE - SERPENT_COIL_RADIUS) * t;
            let a = t * std::f32::consts::TAU * SERPENT_COIL_LAPS;
            (centre.0 + a.cos() * r, centre.1 + a.sin() * r * 0.55)
        }
        // The tail does the work; the head holds station so the arc has a
        // fixed pivot the player can read.
        SerpentAct::TailSweep => centre,
        // Hunt: aim ahead of the player so the body sweeps where they are going.
        _ => (player.0 + vel.0 * SERPENT_LEAD * 60.0, player.1 + vel.1 * SERPENT_LEAD * 60.0),
    }
}

/// Where the serpent wanders to between attacks.
///
/// Waypoints on a flattened ring around the arena, re-picked on arrival or on a
/// timeout. Deliberately NOT influenced by the player: the lull between attacks
/// is the window the player has to get hits in, and a head that keeps hunting
/// through it means there is no such window — the fight has an attack gap on
/// paper and none in play.
///
/// The turn-rate limit does the smoothing, so a handful of straight waypoints
/// come out as a continuous curving patrol rather than a polyline.
pub(crate) fn serpent_roam_goal(
    roam_to: &mut Option<(f32, f32)>,
    roam_ticks: &mut u32,
    seed: &mut u64,
    centre: (f32, f32),
    head: (f32, f32),
) -> (f32, f32) {
    let repick = match *roam_to {
        None => true,
        Some(g) => {
            *roam_ticks = roam_ticks.saturating_sub(1);
            let d2 = (head.0 - g.0).powi(2) + (head.1 - g.1).powi(2);
            d2 < SERPENT_ROAM_ARRIVE * SERPENT_ROAM_ARRIVE || *roam_ticks == 0
        }
    };
    if repick {
        let ang = lcg_range(seed, 0.0, std::f32::consts::TAU);
        let r = lcg_range(seed, SERPENT_ROAM_MIN_R, SERPENT_ROAM_MAX_R);
        *roam_to = Some((
            centre.0 + ang.cos() * r,
            (centre.1 + ang.sin() * r * SERPENT_ROAM_Y_SQUASH)
                .clamp(SERPENT_Y_MIN, SERPENT_Y_MAX),
        ));
        *roam_ticks = SERPENT_ROAM_MAX_TICKS;
    }
    roam_to.unwrap_or(centre)
}

/// The head's current position, for frames where it must not move.
fn return_head_position(s: &State, fallback: (f32, f32)) -> (f32, f32) {
    s.serpent_trail.first().map(|&(x, y, _)| (x, y)).unwrap_or(fallback)
}

/// Override the head's goal during a burrow: it steers into the portal ahead
/// rather than at the player, or it never reaches the thing it is diving into.
fn serpent_rift_goal(s: &State, fallback: (f32, f32)) -> (f32, f32) {
    match s.serpent_rift_phase {
        RiftPhase::Approach => s.serpent_next_hole.unwrap_or(fallback),
        // Pinned in the hole while the body follows it in.
        RiftPhase::Swallow => s.serpent_hole.unwrap_or(fallback),
        // Nowhere to steer while it does not exist.
        RiftPhase::Underground => fallback,
        RiftPhase::Emerge => fallback,
    }
}

// ── The fight ────────────────────────────────────────────────────────────────

pub(crate) fn tick_serpent(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let s = st.lock().unwrap();
        if !s.boss_active || s.dead || s.boss_stasis_active { return; }
    }

    // ── Appearance (once) ────────────────────────────────────────────────
    let spawned = { st.lock().unwrap().boss_spawned };
    if !spawned {
        let name = { let mut s = st.lock().unwrap(); s.boss_spawned = true; s.boss_kind.name() };
        let cx = arena_center_x(c);
        // The anchor square is invisible: the body IS the boss, exactly as for
        // the Colossus. It stays alive only so the off-screen arrow has
        // something to point at.
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, BOSS_Y_CENTER - BOSS_SIZE * 0.5);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(130, 255, 170, 255), 1000.0 * sc,
                )));
            }
        }
        let mut s = st.lock().unwrap();
        s.serpent_goal = (cx, BOSS_Y_CENTER);
        s.serpent_trail.clear();
        s.serpent_arc = 0.0;
        // Seed the trail so the body has somewhere to be on frame one rather
        // than every piece stacking on the head.
        for i in 0..SERPENT_TRAIL_LEN {
            let x = cx + i as f32 * 4.0;
            s.serpent_trail.push((x, BOSS_Y_CENTER, -(i as f32) * 4.0));
        }
    }

    let (px, py, pvx, pvy) = { let s = st.lock().unwrap(); (s.px, s.py, s.vx, s.vy) };
    let centre = (arena_center_x(c), BOSS_Y_CENTER);
    let buffed = { st.lock().unwrap().player_buff > 0 };

    // ── Act director ─────────────────────────────────────────────────────
    let act = {
        let mut s = st.lock().unwrap();
        // Advance the entry clock HERE. It is only incremented inside the Sun
        // Devourer's appearance tick, which no other boss runs — so for the
        // Serpent it would sit at zero forever and the grace period below would
        // never end, leaving the boss permanently unaware of the player.
        s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
        if s.serpent_contact_cooldown > 0 { s.serpent_contact_cooldown -= 1; }
        s.serpent_act_ticks += 1;
        if s.serpent_cooldown > 0 { s.serpent_cooldown -= 1; }

        // Grace at the start of the fight: the serpent cruises without hunting
        // or attacking, so the player gets to read the body's shape before
        // having to plan around it. `boss_entry_ticks` counts from arena entry.
        let noticed = s.boss_entry_ticks >= SERPENT_NOTICE_TICKS;
        if s.serpent_act_ticks >= serpent_act_len(s.serpent_act) {
            if noticed && s.serpent_act == SerpentAct::Prowl && s.serpent_cooldown == 0 {
                let acts = serpent_acts(&s.boss_parts);
                if !acts.is_empty() {
                    let pick = ((lcg(&mut s.seed) * acts.len() as f32) as usize).min(acts.len() - 1);
                    s.serpent_act = acts[pick];
                    s.serpent_act_ticks = 0;
                    s.serpent_rifts.clear();
                    // Where the coil will close. Captured at commitment so the
                    // spiral has a fixed target to tighten onto.
                    s.serpent_coil_at = (s.px, s.py);
                }
            } else {
                // Back to prowling, and hold there for a beat so the player
                // gets navigable time between attacks.
                s.serpent_act = SerpentAct::Prowl;
                s.serpent_act_ticks = 0;
                s.serpent_cooldown = SERPENT_ATTACK_GAP;
                s.serpent_rifts.clear();
                s.serpent_tail_out = false;
                s.serpent_gambit_react = 0;
                s.serpent_gambit_exit = None;
                s.serpent_gambit_exit_ticks = 0;
                s.serpent_lash_from = None;
                // A rift left set would keep hiding every piece that passes
                // through its radius for the rest of the fight.
                s.serpent_hole = None;
                s.serpent_next_hole = None;
                s.serpent_surfaced = 0;
            }
        }
        s.serpent_act
    };
    let act_ticks = { st.lock().unwrap().serpent_act_ticks };

    // ── Head steering ────────────────────────────────────────────────────
    // Turn-rate limited toward a goal rather than snapping to it: a head that
    // can turn instantly makes the body's path unreadable, and the body's path
    // is the actual threat.
    let head = {
        let mut s = st.lock().unwrap();

        // THE HEAD IS IN THE HOLE — the steering code does not touch it.
        //
        // Steering it AT the portal is not the same as stopping it there: it
        // swam straight through, kept going, turned around because the portal
        // was still its goal, and came back — entering twice per cycle with a
        // loop in between. Nothing about a goal makes a thing stop.
        //
        // `Swallow` is then the only writer of the head's position, and it walks
        // it the last stretch to the portal's CENTRE before pinning it. Every
        // piece follows the head's recorded path, so wherever the head stops is
        // where the body disappears; frozen at the commit threshold, the serpent
        // dived into a point off to one side of the hole it was entering.
        // After that the body keeps sliding in on its own, because `Swallow`
        // advances `serpent_arc` and the trail window moves forward through the
        // path the head already laid down.
        let frozen = act == SerpentAct::RiftStrikes
            && matches!(
                s.serpent_rift_phase,
                RiftPhase::Swallow | RiftPhase::Underground
            );
        if frozen {
            return_head_position(&s, centre)
        } else {

        let goal = if s.boss_entry_ticks < SERPENT_NOTICE_TICKS {
            // Not hunting yet: drift a wide, obvious circuit so the player can
            // watch the body move before it comes for them.
            let a = s.boss_entry_ticks as f32 * 0.012;
            (centre.0 + a.cos() * 2600.0, centre.1 + a.sin() * 1500.0)
        } else if act == SerpentAct::Prowl {
            // BETWEEN attacks it wanders instead of hunting. Hunting through
            // the lull meant the head was closing on the player during the only
            // window they had to damage the body — the fight had an attack gap
            // in the code and none in play.
            let cur = s.serpent_trail.first().map(|&(x, y, _)| (x, y)).unwrap_or(centre);
            let (mut to, mut left, mut seed) =
                (s.serpent_roam_to, s.serpent_roam_ticks, s.seed);
            let g = serpent_roam_goal(&mut to, &mut left, &mut seed, centre, cur);
            s.serpent_roam_to = to;
            s.serpent_roam_ticks = left;
            s.seed = seed;
            g
        } else {
            // The coil spirals around where the player was; everything else
            // works off the arena centre.
            let about = if act == SerpentAct::Coil { s.serpent_coil_at } else { centre };
            let g = serpent_goal_for(act, act_ticks, (px, py), about, (pvx, pvy));
            if act == SerpentAct::RiftStrikes { serpent_rift_goal(&s, g) } else { g }
        };
        // Keep the goal inside the band the head is allowed to occupy, so the
        // head is never steering toward somewhere it will then be clamped out
        // of — which would leave it pinned against the limit.
        let goal = (goal.0, goal.1.clamp(SERPENT_Y_MIN, SERPENT_Y_MAX));
        s.serpent_goal = goal;
        let cur = s.serpent_trail.first().map(|&(x, y, _)| (x, y)).unwrap_or(centre);
        let heading = s.boss_phase;
        let want = (goal.1 - cur.1).atan2(goal.0 - cur.0);
        let mut delta = want - heading;
        while delta > std::f32::consts::PI { delta -= std::f32::consts::TAU; }
        while delta < -std::f32::consts::PI { delta += std::f32::consts::TAU; }
        let max_turn = SERPENT_TURN_RATE.to_radians();
        s.boss_phase = heading + delta.clamp(-max_turn, max_turn);

        // Dashing overrides cruise speed — the bite is the one time the head
        // out-runs the player, and it is the payoff for a landed gambit.
        let dashing = act == SerpentAct::WormholeGambit && s.serpent_gambit_react == 0
            && act_ticks > SERPENT_TELEGRAPH_TICKS;
        let speed = if dashing { SERPENT_DASH_SPEED } else { SERPENT_HEAD_SPEED };
        let h = s.boss_phase;
        let next = (cur.0 + h.cos() * speed, cur.1 + h.sin() * speed);
        // The body is TETHERABLE, so a player riding a segment goes wherever
        // the serpent goes. Unclamped, the head wandered toward the death floor
        // and took them into it — killed by the thing they were correctly using
        // as traversal. Clamping the head clamps the body, because the body
        // retraces the head's path.
        let next = (next.0, next.1.clamp(SERPENT_Y_MIN, SERPENT_Y_MAX));
        let mut trail = std::mem::take(&mut s.serpent_trail);
        let mut arc = s.serpent_arc;
        serpent_push_trail(&mut trail, &mut arc, next);
        s.serpent_trail = trail;
        s.serpent_arc = arc;
        next

        }
    };

    // Keep the anchor under the head so the off-screen arrow points at the
    // thing the player is looking for.
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (head.0 - BOSS_SIZE * 0.5, head.1 - BOSS_SIZE * 0.5);
    }

    // ── Shield band ──────────────────────────────────────────────────────
    // The band wraps around the body that is actually there. Wrapping at the
    // ORIGINAL length would park it on dead links for part of every lap, which
    // reads as the shield randomly taking a break.
    let (band, span) = {
        let mut s = st.lock().unwrap();
        let span = serpent_chain_span(&s.boss_parts);
        s.serpent_band = (s.serpent_band + SERPENT_SHIELD_SPEED / 60.0).rem_euclid(span as f32);
        (s.serpent_band, span)
    };

    // The lash moves head and tail OFF the body's path, so it has to be
    // resolved before anything reads their positions — the visuals, the contact
    // test and the spine all have to agree about where they are.
    let lash = if act == SerpentAct::SpineLash {
        serpent_lash_positions(st, act_ticks, centre)
    } else {
        st.lock().unwrap().serpent_lash_from = None;
        None
    };

    // ── Place every piece, and collect what can be hit or hit you ────────
    // `size` is the piece's authored size and drives the image cache and the
    // hit radii; `draw` is how big it is on screen this frame, which shrinks to
    // nothing as it funnels into a rift. Keeping them apart is what stops the
    // funnel from rasterising a fresh sprite at every intermediate pixel size.
    struct Piece { idx: usize, id: &'static str, pos: (f32, f32), rot: f32,
                   size: f32, draw: f32, shielded: bool, seam: f32, open: bool }
    let pieces: Vec<Piece> = {
        let s = st.lock().unwrap();
        let mut out = Vec::new();
        for (i, p) in s.boss_parts.iter().enumerate() {
            if !p.alive { continue; }
            let chain = serpent_chain_index(&s.boss_parts, i, p.id);
            let placed = match (lash, p.id) {
                // During a lash the two survivors are choreographed, not
                // trailing: they face along the spine so the sweep reads as one
                // object rather than two things that happen to be connected.
                (Some((h, t)), "head") => {
                    Some((h, (h.1 - t.1).atan2(h.0 - t.0).to_degrees()))
                }
                (Some((h, t)), "tail") => {
                    Some((t, (t.1 - h.1).atan2(t.0 - h.0).to_degrees()))
                }
                _ => serpent_trail_point(
                    &s.serpent_trail, s.serpent_arc, serpent_chain_distance(chain),
                ),
            };
            let Some((pos, rot)) = placed else { continue; };
            // Completely gone between portals.
            if act == SerpentAct::RiftStrikes
                && s.serpent_rift_phase == RiftPhase::Underground
            {
                continue;
            }
            let (size, id) = match p.id {
                "head" => (SERPENT_HEAD_SIZE, "head"),
                "tail" => (SERPENT_TAIL_SIZE, "tail"),
                _ => (SERPENT_SEGMENT_SIZE, "seg"),
            };
            // Inside the active rift: not on screen, and not hittable in
            // either direction. One rule covers the whole burrow — diving, a
            // piece funnels down to the mouth and vanishes; emerging, it grows
            // back out of it — so the body goes in head-first one piece at a
            // time and comes back out the same way with no per-piece animation.
            //
            // The funnel runs to the rift's CENTRE, not to its rim. Hiding a
            // piece the moment it crossed the rim made the serpent wink out at
            // the edge of the hole and pop back in at the edge on the way out,
            // which reads as the portal deleting it rather than swallowing it.
            let mut draw = size;
            if let Some(hole) = s.serpent_hole {
                let d = ((pos.0 - hole.0).powi(2) + (pos.1 - hole.1).powi(2)).sqrt();
                if d < SERPENT_RIFT_MOUTH_R {
                    continue;
                }
                if d < SERPENT_RIFT_THROAT_R {
                    let t = (d - SERPENT_RIFT_MOUTH_R)
                        / (SERPENT_RIFT_THROAT_R - SERPENT_RIFT_MOUTH_R);
                    draw = size * t.clamp(0.0, 1.0);
                }
            }
            // A shielded PART (phase gating) is never open. The travelling band
            // is a second, independent gate on top of that.
            let banded = p.id == "seg" && serpent_shielded_now(chain, band, span);
            out.push(Piece {
                idx: i, id, pos, rot, size, draw,
                shielded: p.shielded || banded,
                seam: if p.shielded { 0.0 } else { serpent_seam(chain, band, span) },
                // Hittable only once the plating has actually opened. The glow
                // below reads this SAME flag, so the cue and the hit box can
                // never disagree — keyed to the raw band boolean, the glow lit
                // while the armour was still visibly shut.
                open: !p.shielded && serpent_seam(chain, band, span) >= SERPENT_OPEN_AT,
            });
        }
        out
    };

    for piece in &pieces {
        let name = format!("serpent_part_{}", piece.idx);
        // Placed from the DRAWN size so a funnelling piece stays centred on its
        // trail point while it shrinks, instead of sliding toward its own
        // top-left corner as the sprite gets smaller.
        let half = piece.draw * 0.5;
        let seam_step = (piece.seam * crate::images::SERPENT_SEAM_STEPS as f32).round() as u32;
        let img = match piece.id {
            "head" => crate::images::serpent_head_cached(piece.size as u32),
            "tail" => crate::images::serpent_tail_cached(
                piece.size as u32,
                if act == SerpentAct::TailLaunch { crate::images::SERPENT_SEAM_STEPS } else { seam_step },
            ),
            _ => crate::images::serpent_segment_cached(piece.size as u32, seam_step),
        };
        if let Some(obj) = c.get_game_object_mut(&name) {
            // Ellipse, not Rectangle: `GameObject::highlight_shape` copies the
            // drawable's ShapeType, so a rectangular sprite gets a rectangular
            // glow. Every serpent piece is drawn inside a round silhouette, so
            // the ellipse both clips correctly and gives the round glow.
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (piece.draw, piece.draw), piece.rot),
                image: img, color: None,
            });
            obj.size = (piece.draw, piece.draw);
            obj.rotation = piece.rot;
            obj.position = (piece.pos.0 - half, piece.pos.1 - half);
            obj.visible = true;
            if piece.open {
                obj.set_glow(GlowConfig { color: Color(255, 190, 70, 190), width: 30.0 });
            } else {
                obj.clear_glow();
            }
        }
        // The energised forcefield, attached so it takes the segment's own
        // depth and position — see `fx::attach_mega_fx`.
        if piece.shielded && piece.id == "seg" {
            let energy = 1.0 - piece.seam;
            crate::scenes::game::fx::attach_mega_fx(
                c, &name, crate::scenes::game::fx::flat_white(),
                (piece.draw * 1.12, piece.draw * 1.12),
                (0.45, 1.0, 0.75, energy),
                [MEGA_BIT_SEGMENT_SHIELD, 0, 0, 0], 1,
            );
        } else {
            crate::scenes::game::fx::clear_object_fx(c, &name);
        }
    }
    // Hide any destroyed piece's object and release its shield.
    {
        let live: Vec<usize> = pieces.iter().map(|p| p.idx).collect();
        let total = { st.lock().unwrap().boss_parts.len() };
        for i in 0..total {
            if live.contains(&i) { continue; }
            let name = format!("serpent_part_{i}");
            crate::scenes::game::fx::clear_object_fx(c, &name);
            if let Some(obj) = c.get_game_object_mut(&name) { obj.visible = false; }
        }
    }

    tick_serpent_attacks(c, st, act, act_ticks, head, (px, py), centre, lash);

    // ── Damage: buffed hits on an exposed piece ──────────────────────────
    if buffed {
        let mut killed = false;
        let mut s = st.lock().unwrap();
        for piece in &pieces {
            if !piece.open { continue; }
            let r = piece.draw * SERPENT_CONTACT_R + PLAYER_R;
            if (px - piece.pos.0).powi(2) + (py - piece.pos.1).powi(2) < r * r {
                if s.boss_part_invuln_ticks == 0 {
                    let p = &mut s.boss_parts[piece.idx];
                    p.hp -= 1;
                    if p.hp <= 0 {
                        p.alive = false;
                        s.boss_part_invuln_ticks = COLOSSUS_PART_INVULN_TICKS;
                        killed = true;
                    }
                    s.buff_hit_flash = 20;
                }
                break;
            }
        }
        let _ = killed;
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
    } else {
        let mut s = st.lock().unwrap();
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
    }

    // ── Body contact ─────────────────────────────────────────────────────
    // Unbuffed: a heart and a hard throw. Buffed: thrown and untethered but not
    // hurt, spending one absorption — the same rule the Colossus uses, so the
    // buff means one thing across the whole game.
    let contact = {
        let s = st.lock().unwrap();
        if s.serpent_contact_cooldown > 0 || s.dead { None } else {
            pieces.iter().find(|piece| {
                let r = piece.draw * SERPENT_CONTACT_R + PLAYER_R;
                (px - piece.pos.0).powi(2) + (py - piece.pos.1).powi(2) < r * r
            }).map(|piece| (piece.pos, piece.id))
        }
    };
    if let Some((at, id)) = contact {
        // Only the HEAD cuts the rope on contact. Brushing a segment or the
        // tail is riding the level badly, not being attacked — and the body is
        // the traversal, so cutting the rope there strands the player on the
        // thing that just hit them.
        let cut = serpent_cut_for(if id == "head" {
            Strike::HeadContact
        } else {
            Strike::BodyContact
        });
        serpent_strike_player(c, st, at, buffed, cut);
    }

    // ── Phase gating + win ───────────────────────────────────────────────
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_parts.len() {
            let prev_dead = i > 0 && s.boss_parts[..i].iter().all(|p| !p.alive);
            if prev_dead {
                let part = &mut s.boss_parts[i];
                if part.alive && part.shielded {
                    part.shielded = false;
                    // A part that has just become reachable has not attacked, so
                    // it must not inherit an open post-attack window.
                    part.post_attack = false;
                }
            }
        }
        s.boss_hp = boss_total_hp(&s);
    }
    let dead = { let s = st.lock().unwrap(); s.boss_parts.iter().all(|p| !p.alive) };
    if dead {
        for i in 0..pieces.len().max(SERPENT_SEGMENTS + 2) {
            crate::scenes::game::fx::clear_object_fx(c, &format!("serpent_part_{i}"));
            if let Some(obj) = c.get_game_object_mut(&format!("serpent_part_{i}")) {
                obj.visible = false;
            }
        }
        serpent_clear_rifts(c, st);
        finish_boss(c, st);
    }
}

/// Whether a strike also cuts the player's rope.
///
/// Only the serpent's DELIBERATE strikes do: the tail whip, the launched tail,
/// the bite, and touching the head itself. Everything else — brushing a
/// segment, standing where the body erupts, catching the spine — throws the
/// player without untethering them.
///
/// Untethering on every hit made the fight unplayable in a way that looked like
/// a difficulty problem and was really a mechanic problem: the body IS the
/// level here, so a hit that cuts the rope does not just cost a heart, it takes
/// away the traversal the player was using to get to the thing they were
/// aiming at. Two punishments arrived as one, on contact with an eight-piece
/// object filling the arena.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Cut {
    /// Throw only. The rope survives.
    Keep,
    /// Throw and cut the rope.
    Rope,
}

/// Everything in the fight that can hit the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Strike {
    /// The tail whipped through the play band.
    TailSweep,
    /// The detached tail, homing.
    TailLaunch,
    /// The head's bite at the end of a gambit.
    Bite,
    /// Touching the head itself.
    HeadContact,
    /// Touching a segment or the tail as part of the body.
    BodyContact,
    /// Standing where the serpent came out of a rift.
    RiftEruption,
    /// The energy spine swept between head and tail.
    SpineLash,
}

/// Whether a strike cuts the rope.
///
/// One function rather than a decision at each call site, so adding an attack
/// forces a choice here instead of inheriting whatever the last one did — which
/// is how every hit in the fight ended up untethering.
pub(crate) fn serpent_cut_for(source: Strike) -> Cut {
    match source {
        // AIMED. The serpent chose the player and connected.
        Strike::TailSweep | Strike::TailLaunch | Strike::Bite | Strike::HeadContact => Cut::Rope,
        // INCIDENTAL. The body moved and the player was there. Being thrown is
        // the whole punishment; the rope holds.
        Strike::BodyContact | Strike::RiftEruption | Strike::SpineLash => Cut::Keep,
    }
}

/// Throw the player off the body, and cost them a heart if they are unprotected.
fn serpent_strike_player(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    at: (f32, f32),
    buffed: bool,
    cut: Cut,
) {
    let (px, py) = { let s = st.lock().unwrap(); (s.px, s.py) };
    let dx = px - at.0;
    let dy = py - at.1;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    let push = (dx / d * SERPENT_CONTACT_PUSH, dy / d * SERPENT_CONTACT_PUSH);
    {
        let mut s = st.lock().unwrap();
        s.serpent_contact_cooldown = SERPENT_CONTACT_COOLDOWN;
        s.vx = push.0;
        s.vy = push.1;
        if cut == Cut::Rope {
            s.hooked = false;
            s.active_hook = String::new();
        }
    }
    if cut == Cut::Rope {
        c.run(Action::Hide { target: Target::name("rope") });
    }
    if let Some(obj) = c.get_game_object_mut("player") { obj.momentum = push; }
    c.set_var("boss_knockback_ticks", Value::I32(SERPENT_KNOCKBACK_TICKS));

    if buffed {
        let mut s = st.lock().unwrap();
        if s.player_buff > 0 {
            s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
            if s.buff_absorbs == 0 { s.player_buff = 0; s.buff_timer = 0; }
        }
    } else {
        let dead = { st.lock().unwrap().dead };
        if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
    }
}

// ── Attacks ──────────────────────────────────────────────────────────────────

/// Hide every rift marker. Called when an act ends and when the fight does — a
/// rift left visible would keep teleporting a player into a boss that no longer
/// exists.
pub(crate) fn serpent_clear_rifts(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.serpent_rifts.clear();
        s.serpent_gambit_exit = None;
        s.serpent_gambit_exit_ticks = 0;
    }
    for i in 0..SERPENT_RIFT_SLOTS {
        if let Some(obj) = c.get_game_object_mut(&format!("serpent_rift_{i}")) {
            obj.visible = false;
        }
    }
    if let Some(obj) = c.get_game_object_mut("serpent_spine") { obj.visible = false; }
}

pub(crate) fn tick_serpent_attacks(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    act: SerpentAct,
    ticks: u32,
    head: (f32, f32),
    player: (f32, f32),
    centre: (f32, f32),
    lash: Option<((f32, f32), (f32, f32))>,
) {
    let _ = centre;
    match act {
        SerpentAct::Prowl | SerpentAct::Coil => serpent_clear_rifts(c, st),
        SerpentAct::RiftStrikes => serpent_rift_strikes(c, st, ticks, player),
        SerpentAct::TailSweep => serpent_tail_sweep(c, st, ticks, player, lash),
        SerpentAct::TailLaunch => serpent_tail_launch(c, st, ticks, player),
        SerpentAct::SpineLash => serpent_spine_lash(c, st, ticks, player, lash),
        SerpentAct::WormholeGambit => serpent_wormhole_gambit(c, st, ticks, head, player),
    }
}

/// Show a rift marker, growing as it nears eruption so the wind-up is legible.
fn draw_rift(c: &mut Canvas, i: usize, x: f32, y: f32, charge: f32, r: f32) {
    let name = format!("serpent_rift_{i}");
    let rr = r * (0.35 + 0.65 * charge);
    if let Some(obj) = c.get_game_object_mut(&name) {
        // The shipped wormhole animation, which is what the arena warps already
        // use — a rift the serpent travels through should look like the rifts
        // the player travels through, and it saves authoring a second one.
        if obj.animated_sprite.is_none() {
            obj.set_animation(wormhole2_template());
        }
        obj.size = (rr * 2.0, rr * 2.0);
        obj.position = (x - rr, y - rr);
        obj.visible = true;
    }
}

// ── RIFT STRIKES ─────────────────────────────────────────────────────────────
//
// The head dives and erupts from a sequence of telegraphed points. The research
// version of this is a sand-worm burrowing; in space it goes through rifts.
//
// The points are seeded around the player at the moment the act begins, so the
// sequence is fixed and learnable rather than chasing them — a homing burrow is
// unreadable, and this attack's whole appeal is reading it.
/// Total body length in trail arc — how far the head must travel for the whole
/// serpent to pass a point.
///
/// `span` is the number of LIVING links (see `serpent_chain_span`), so a
/// half-eaten serpent finishes its dive as soon as the tail is actually under
/// rather than waiting out the length of a body it no longer has.
///
/// The `SWALLOW_R` term covers the head's own run-in: the burrow commits at that
/// distance and the head then walks the rest of the way to the centre, so that
/// stretch is arc the body has to pay before the tail even starts to disappear.
pub(crate) fn serpent_body_arc_for(span: usize) -> f32 {
    span.saturating_sub(1) as f32 * SERPENT_SEGMENT_SPACING + SERPENT_RIFT_SWALLOW_R
}

/// The full-length body arc, for sizing the act's worst-case ceiling.
pub(crate) fn serpent_body_arc() -> f32 {
    serpent_body_arc_for(SERPENT_SEGMENTS + 2)
}

fn serpent_rift_strikes(c: &mut Canvas, st: &Arc<Mutex<State>>, ticks: u32, player: (f32, f32)) {
    // Approach -> Swallow -> Emerge, repeating. Each stage ends on the thing it
    // is waiting for rather than on a tick count, because the head has to
    // actually REACH the portal before the body can go in — a fixed clock let
    // the head wander off and the dive never happened on screen.
    if ticks < SERPENT_TELEGRAPH_TICKS {
        // Wind-up: open the first portal ahead of the head.
        let mut s = st.lock().unwrap();
        if s.serpent_next_hole.is_none() {
            let head = serpent_trail_point(&s.serpent_trail, s.serpent_arc, 0.0)
                .map(|(p, _)| p)
                .unwrap_or(player);
            let h = s.boss_phase;
            s.serpent_next_hole = Some((
                head.0 + h.cos() * SERPENT_RIFT_AHEAD,
                (head.1 + h.sin() * SERPENT_RIFT_AHEAD).clamp(SERPENT_Y_MIN, SERPENT_Y_MAX),
            ));
            s.serpent_rift_phase = RiftPhase::Approach;
            s.serpent_rift_ticks = 0;
            s.serpent_surfaced = 0;
            s.serpent_hole = None;
        }
        let next = s.serpent_next_hole;
        drop(s);
        if let Some((x, y)) = next {
            let charge = (ticks as f32 / SERPENT_TELEGRAPH_TICKS as f32).clamp(0.0, 1.0);
            draw_rift(c, 1, x, y, charge, SERPENT_RIFT_R);
        }
        return;
    }

    let head_pos = {
        let s = st.lock().unwrap();
        serpent_trail_point(&s.serpent_trail, s.serpent_arc, 0.0).map(|(p, _)| p)
    };
    let Some(head_pos) = head_pos else { return; };

    let mut erupted: Option<(f32, f32)> = None;
    {
        let mut s = st.lock().unwrap();
        s.serpent_rift_ticks += 1;
        match s.serpent_rift_phase {
            // Swimming into the portal ahead. Ends when the head arrives, so
            // the swim is always visible however long it takes.
            RiftPhase::Approach => {
                if let Some(hole) = s.serpent_next_hole {
                    let d = ((head_pos.0 - hole.0).powi(2) + (head_pos.1 - hole.1).powi(2)).sqrt();
                    if d < SERPENT_RIFT_SWALLOW_R * 0.6 {
                        // The head is in. From here the body slides after it.
                        s.serpent_hole = Some(hole);
                        s.serpent_next_hole = None;
                        s.serpent_rift_phase = RiftPhase::Swallow;
                        s.serpent_rift_ticks = 0;
                    }
                }
            }
            // The head finishes its dive to the CENTRE of the hole, and the
            // trail window then slides forward, pulling each piece down the
            // same path in turn.
            //
            // Walking the head in rather than freezing it where it committed is
            // what puts the whole body through the middle of the portal: every
            // piece follows the head's recorded path, so wherever the head
            // stopped is where the body vanishes. Stopping it at the commit
            // threshold left the serpent diving into a point off to one side of
            // the hole it was supposedly entering.
            RiftPhase::Swallow => {
                let hole = s.serpent_hole;
                let mut walked = false;
                if let Some(hole) = hole {
                    let dx = hole.0 - head_pos.0;
                    let dy = hole.1 - head_pos.1;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 1.0 {
                        let step = SERPENT_RIFT_SWALLOW_SPEED.min(d);
                        let next = (head_pos.0 + dx / d * step, head_pos.1 + dy / d * step);
                        let mut trail = std::mem::take(&mut s.serpent_trail);
                        let mut arc = s.serpent_arc;
                        // The push advances the arc by exactly the distance
                        // moved, which is the same budget the pinned branch adds
                        // by hand — so `swallowed` below stays an accurate count
                        // of arc consumed either way.
                        serpent_push_trail(&mut trail, &mut arc, next);
                        s.serpent_trail = trail;
                        s.serpent_arc = arc;
                        walked = true;
                    }
                }
                if !walked {
                    s.serpent_arc += SERPENT_RIFT_SWALLOW_SPEED;
                }
                let swallowed = s.serpent_rift_ticks as f32 * SERPENT_RIFT_SWALLOW_SPEED;
                if swallowed >= serpent_body_arc_for(serpent_chain_span(&s.boss_parts)) {
                    // Fully under. CLOSE the entry and show nothing at all for
                    // a beat — opening the exit on this frame made the serpent
                    // look like it came straight back out of the hole it had
                    // just gone into.
                    s.serpent_hole = None;
                    s.serpent_next_hole = None;
                    s.serpent_rift_phase = RiftPhase::Underground;
                    s.serpent_rift_ticks = 0;
                }
            }
            // Absent. Nothing on screen; the exit opens when the beat is up,
            // aimed at wherever the player has moved to BY THEN — which is what
            // makes running during the gap worth doing.
            RiftPhase::Underground => {
                if s.serpent_rift_ticks >= SERPENT_RIFT_HIDDEN {
                    let ang = lcg_range(&mut s.seed, 0.0, std::f32::consts::TAU);
                    let off = lcg_range(&mut s.seed, 600.0, 1400.0);
                    let hole = (
                        player.0 + ang.cos() * off,
                        (player.1 + ang.sin() * off * 0.6).clamp(SERPENT_Y_MIN, SERPENT_Y_MAX),
                    );
                    let heading = (player.1 - hole.1).atan2(player.0 - hole.0);
                    s.boss_phase = heading;
                    s.serpent_hole = Some(hole);
                    s.serpent_trail.clear();
                    s.serpent_arc = 0.0;
                    s.serpent_trail.push((hole.0, hole.1, 0.0));
                    s.serpent_rift_phase = RiftPhase::Emerge;
                    s.serpent_rift_ticks = 0;
                    s.serpent_surfaced += 1;
                    erupted = Some(hole);
                }
            }
            // Climbing out. Ends when the tail clears the portal, so every
            // piece is visibly paid out of it.
            RiftPhase::Emerge => {
                if s.serpent_arc >= serpent_body_arc_for(serpent_chain_span(&s.boss_parts)) {
                    s.serpent_hole = None;
                    if s.serpent_surfaced < SERPENT_RIFT_COUNT
                        && s.serpent_rift_ticks >= SERPENT_RIFT_SURFACE
                    {
                        // Out, and the window has passed: open the next portal
                        // ahead and go again.
                        let h = s.boss_phase;
                        s.serpent_next_hole = Some((
                            head_pos.0 + h.cos() * SERPENT_RIFT_AHEAD,
                            (head_pos.1 + h.sin() * SERPENT_RIFT_AHEAD)
                                .clamp(SERPENT_Y_MIN, SERPENT_Y_MAX),
                        ));
                        s.serpent_rift_phase = RiftPhase::Approach;
                        s.serpent_rift_ticks = 0;
                    }
                }
            }
        }
    }

    // The eruption strikes anyone standing on the exit.
    if let Some(hole) = erupted {
        let (px, py, buffed) = { let s = st.lock().unwrap(); (s.px, s.py, s.player_buff > 0) };
        let r = SERPENT_RIFT_R + PLAYER_R;
        if (px - hole.0).powi(2) + (py - hole.1).powi(2) < r * r {
            // Erupting under someone is the body moving, not a strike aimed
            // at them: thrown, but the rope holds.
            serpent_strike_player(c, st, hole, buffed, serpent_cut_for(Strike::RiftEruption));
        }
    }

    // Rift 0 is the one the body is in — fully open, there is a serpent in it.
    // Rift 1 is the portal ahead, widening as the head swims toward it.
    let (hole, next, phase, pticks) = {
        let s = st.lock().unwrap();
        (s.serpent_hole, s.serpent_next_hole, s.serpent_rift_phase, s.serpent_rift_ticks)
    };
    if let Some((x, y)) = hole {
        draw_rift(c, 0, x, y, 1.0, SERPENT_RIFT_R);
    } else if let Some(obj) = c.get_game_object_mut("serpent_rift_0") {
        obj.visible = false;
    }
    if let Some((x, y)) = next {
        let charge = if phase == RiftPhase::Approach {
            (pticks as f32 / 40.0).clamp(0.35, 1.0)
        } else {
            1.0
        };
        draw_rift(c, 1, x, y, charge, SERPENT_RIFT_R);
    } else if let Some(obj) = c.get_game_object_mut("serpent_rift_1") {
        obj.visible = false;
    }
}

// ── TAIL SWEEP ───────────────────────────────────────────────────────────────
//
// The head holds station and the tail whips a wide arc through the play band.
//
// It is the only attack in the fight answered by ELEVATION — the hands' zones
// are left by moving sideways, the beam by getting off a line, the coil by
// finding a gap. Nothing else asks the player to simply be higher, and a
// grapple game should ask that at least once.
fn serpent_tail_sweep(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    ticks: u32,
    player: (f32, f32),
    lash: Option<((f32, f32), (f32, f32))>,
) {
    let _ = lash;
    // Pivot at the TAIL'S BASE — the living piece nearest it — not at the head.
    // A tail hinged on the head sweeps a line the length of the whole body,
    // which is not a tail whip, it is the serpent rotating. Anchoring at the
    // adjacent piece makes the arc the tail's own reach, and it shortens
    // naturally as segments die.
    let (pivot, tail_idx) = {
        let s = st.lock().unwrap();
        let tail = s.boss_parts.iter().position(|p| p.id == "tail" && p.alive);
        let base = s
            .boss_parts
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive && p.id != "tail")
            .map(|(i, p)| (serpent_chain_index(&s.boss_parts, i, p.id), i, p.id))
            .max_by_key(|(chain, _, _)| *chain)
            .and_then(|(chain, _, _)| {
                serpent_trail_point(
                    &s.serpent_trail, s.serpent_arc, serpent_chain_distance(chain),
                ).map(|(pos, _)| pos)
            });
        (base, tail)
    };
    let (Some(head), Some(tail_idx)) = (pivot, tail_idx) else { return; };

    let charging = ticks < SERPENT_TELEGRAPH_TICKS;
    // Swing from below the band up through it, so the warning is the tail
    // dropping low and the danger is it coming back up.
    let span = SERPENT_SWEEP_TICKS.saturating_sub(SERPENT_TELEGRAPH_TICKS).max(1);
    let t = (ticks.saturating_sub(SERPENT_TELEGRAPH_TICKS) as f32 / span as f32).clamp(0.0, 1.0);
    let ang = if charging {
        // Wind-up: hold at the start of the arc so the sweep's origin is read
        // before it moves.
        -SERPENT_SWEEP_ARC * 0.5
    } else {
        -SERPENT_SWEEP_ARC * 0.5 + SERPENT_SWEEP_ARC * t
    };
    let pos = (
        head.0 + ang.cos() * SERPENT_SWEEP_RADIUS,
        head.1 + ang.sin() * SERPENT_SWEEP_RADIUS * 0.45,
    );

    let half = SERPENT_TAIL_SIZE * 0.5;
    if let Some(obj) = c.get_game_object_mut(&format!("serpent_part_{tail_idx}")) {
        obj.position = (pos.0 - half, pos.1 - half);
        obj.rotation = (pos.1 - head.1).atan2(pos.0 - head.0).to_degrees();
        obj.visible = true;
    }
    // The swept arc, drawn as the spine so the player can see the reach.
    let dx = pos.0 - head.0;
    let dy = pos.1 - head.1;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let th = if charging { SERPENT_SWEEP_THICKNESS * 0.3 } else { SERPENT_SWEEP_THICKNESS };
    let mid = ((head.0 + pos.0) * 0.5, (head.1 + pos.1) * 0.5);
    if let Some(obj) = c.get_game_object_mut("serpent_spine") {
        obj.size = (len, th);
        obj.rotation = dy.atan2(dx).to_degrees();
        obj.position = (mid.0 - len * 0.5, mid.1 - th * 0.5);
        obj.visible = true;
    }
    if charging { return; }

    let cooling = { st.lock().unwrap().serpent_contact_cooldown > 0 };
    if cooling { return; }
    if point_segment_dist(player, head, pos) < th * 0.5 + PLAYER_R {
        let buffed = { st.lock().unwrap().player_buff > 0 };
        // The tail whip. A deliberate strike, so it cuts the rope.
        serpent_strike_player(
            c, st, (player.0, player.1 + 400.0), buffed,
            serpent_cut_for(Strike::TailSweep),
        );
    }
}

// ── TAIL LAUNCH ──────────────────────────────────────────────────────────────
//
// The tail detaches, rockets at the player and reattaches. It launches ITSELF
// rather than firing a body segment, because it is thruster-tipped and because
// an attack that spends segments would run out exactly as the fight reaches the
// phase where segments are gone.
//
// It is the one attack the player removes by progressing: the tail is shielded
// until the body is destroyed, so this is endured for most of the fight and
// only stops once the tail itself is killed.
fn serpent_tail_launch(c: &mut Canvas, st: &Arc<Mutex<State>>, ticks: u32, player: (f32, f32)) {
    let tail_idx = {
        let s = st.lock().unwrap();
        s.boss_parts.iter().position(|p| p.id == "tail" && p.alive)
    };
    let Some(tail_idx) = tail_idx else { return; };
    let name = format!("serpent_part_{tail_idx}");

    if ticks < SERPENT_TELEGRAPH_TICKS {
        // Winding up: the thruster is already lit by the art path above.
        return;
    }

    let launched = { st.lock().unwrap().serpent_tail_out };
    if !launched {
        let from = c.get_game_object(&name)
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
        let Some(from) = from else { return; };
        let mut s = st.lock().unwrap();
        s.serpent_tail_out = true;
        s.serpent_tail_pos = from;
        return;
    }

    // Fly at the player, then hold. The tail is drawn by the body path, so
    // overriding its object here is what makes it visibly leave the body.
    let pos = {
        let mut s = st.lock().unwrap();
        let (tx, ty) = s.serpent_tail_pos;
        let dx = player.0 - tx;
        let dy = player.1 - ty;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let step = SERPENT_TAIL_LAUNCH_SPEED.min(d);
        let next = (tx + dx / d * step, ty + dy / d * step);
        s.serpent_tail_pos = next;
        next
    };
    let half = SERPENT_TAIL_SIZE * 0.5;
    if let Some(obj) = c.get_game_object_mut(&name) {
        obj.position = (pos.0 - half, pos.1 - half);
        obj.visible = true;
    }

    let (px, py, buffed, cooling) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.player_buff > 0, s.serpent_contact_cooldown > 0)
    };
    let r = SERPENT_TAIL_SIZE * SERPENT_CONTACT_R + PLAYER_R;
    if !cooling && (px - pos.0).powi(2) + (py - pos.1).powi(2) < r * r {
        // The launched tail, homing the whole way: it hit because it aimed.
        serpent_strike_player(c, st, pos, buffed, serpent_cut_for(Strike::TailLaunch));
    }
}

/// Where head and tail sit during a lash.
///
/// Two beats. First they LEAVE the body's path and travel to opposite sides —
/// eased, so the move reads as the serpent deliberately taking up a stance
/// rather than teleporting. Then the pair rotates about the arena centre and
/// the spine between them sweeps the floor.
///
/// The endpoints orbit at DIFFERENT radii, which puts the pivot off-centre so
/// the line translates as it rotates. An equal-radius pair is a diameter: it
/// spins about a fixed point, and the safe side is simply whichever half you
/// started in. Off-centre, the safe side moves while you stand in it, which is
/// what makes this a different problem from a beam rather than a longer one.
pub(crate) fn serpent_lash_anchors(
    ticks: u32,
    centre: (f32, f32),
    from: ((f32, f32), (f32, f32)),
) -> ((f32, f32), (f32, f32)) {
    let tele = SERPENT_TELEGRAPH_TICKS.max(1);
    let head_r = SERPENT_LASH_RADIUS;
    let tail_r = SERPENT_LASH_RADIUS * SERPENT_LASH_TAIL_RATIO;
    let at = |ang: f32| {
        (
            (centre.0 + ang.cos() * head_r, centre.1 + ang.sin() * head_r * SERPENT_LASH_Y),
            (centre.0 - ang.cos() * tail_r, centre.1 - ang.sin() * tail_r * SERPENT_LASH_Y),
        )
    };

    if ticks < tele {
        // Taking up the stance. Smoothstep so it accelerates out of the body's
        // path and settles into position instead of arriving at full speed.
        let u = ticks as f32 / tele as f32;
        let t = u * u * (3.0 - 2.0 * u);
        let (h, l) = at(0.0);
        (lerp2(from.0, h, t), lerp2(from.1, l, t))
    } else {
        let span = SERPENT_LASH_TICKS.saturating_sub(tele).max(1);
        let t = ((ticks - tele) as f32 / span as f32).clamp(0.0, 1.0);
        at(t * SERPENT_LASH_ARC)
    }
}

/// Head and tail positions for this frame of a lash, capturing where they were
/// when it began so the move into position is a lerp and not a jump.
///
/// Returns `None` if either piece is missing, which is also what stops the
/// attack drawing a spine to a part that has just been destroyed.
pub(crate) fn serpent_lash_positions(
    st: &Arc<Mutex<State>>,
    ticks: u32,
    centre: (f32, f32),
) -> Option<((f32, f32), (f32, f32))> {
    let mut s = st.lock().unwrap();
    let find = |s: &State, id: &str| {
        s.boss_parts.iter().position(|p| p.id == id && p.alive).and_then(|i| {
            let chain = serpent_chain_index(&s.boss_parts, i, id);
            serpent_trail_point(&s.serpent_trail, s.serpent_arc, serpent_chain_distance(chain))
                .map(|(pos, _)| pos)
        })
    };
    if s.serpent_lash_from.is_none() {
        let h = find(&s, "head")?;
        let t = find(&s, "tail")?;
        s.serpent_lash_from = Some((h, t));
    }
    // Both pieces must still exist, or there is no spine to sweep.
    if find(&s, "head").is_none() || find(&s, "tail").is_none() {
        return None;
    }
    let from = s.serpent_lash_from?;
    Some(serpent_lash_anchors(ticks, centre, from))
}

// ── SPINE LASH ───────────────────────────────────────────────────────────────
//
// With the segments gone, head and tail are joined by an exposed energy spine,
// and the pair sweeps it across the arena.
//
// This is a MOVING LINE BETWEEN TWO MOVING ENDPOINTS, which is a different
// dodge problem from every beam in the game so far — those are rays from a
// fixed origin, where getting off the line once is enough. Here the line's
// angle changes as it sweeps, so the safe side moves while you are on it.
fn serpent_spine_lash(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    ticks: u32,
    player: (f32, f32),
    lash: Option<((f32, f32), (f32, f32))>,
) {
    let Some((a, b)) = lash else { return; };

    let charging = ticks < SERPENT_TELEGRAPH_TICKS;
    let t = ((ticks.saturating_sub(SERPENT_TELEGRAPH_TICKS)) as f32
        / (SERPENT_LASH_TICKS - SERPENT_TELEGRAPH_TICKS).max(1) as f32).clamp(0.0, 1.0);

    // Draw the spine as one rotated strip between the two endpoints.
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let deg = dy.atan2(dx).to_degrees();
    let th = if charging { SERPENT_LASH_THICKNESS * 0.35 } else { SERPENT_LASH_THICKNESS };
    let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    if let Some(obj) = c.get_game_object_mut("serpent_spine") {
        obj.size = (len, th);
        obj.rotation = deg;
        obj.position = (mid.0 - len * 0.5, mid.1 - th * 0.5);
        obj.visible = true;
    }
    if charging { return; }

    // Damaging only once the sweep is live, and once per pass.
    let cooling = { st.lock().unwrap().serpent_contact_cooldown > 0 };
    if cooling { return; }
    let d = point_segment_dist(player, a, b);
    if d < th * 0.5 + PLAYER_R {
        let buffed = { st.lock().unwrap().player_buff > 0 };
        // Thrown perpendicular to the spine, so the escape direction is the one
        // the player can see rather than "away from a point".
        let nx = -dy / len;
        let ny = dx / len;
        let side = if (player.0 - mid.0) * nx + (player.1 - mid.1) * ny >= 0.0 { 1.0 } else { -1.0 };
        let at = (player.0 - nx * side * 100.0, player.1 - ny * side * 100.0);
        // The spine between head and tail, not either of them — caught by the
        // sweep rather than struck, so the rope holds.
        serpent_strike_player(c, st, at, buffed, serpent_cut_for(Strike::SpineLash));
    }
    let _ = t;
}

// ── WORMHOLE GAMBIT ──────────────────────────────────────────────────────────
//
// The head alone. It opens rifts loosely aimed at the player; falling into one
// delivers them directly in front of the head, which is already winding up a
// dash bite. A tether inside the reaction window still saves them.
//
// The head is INVULNERABLE while dashing, so the punish is the recovery — the
// same rule every other part follows. And the delivery is a capture, which is
// the only one in the game: the escape is a tether, so the attack is countered
// with the game's core verb rather than with a dodge.
fn serpent_wormhole_gambit(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    ticks: u32,
    head: (f32, f32),
    player: (f32, f32),
) {
    // ── The exit portal ──────────────────────────────────────────────────
    // Ticked first, because a capture returns early and every other path
    // through this function has to keep the portal on screen and closing.
    {
        let (exit, left) = {
            let mut s = st.lock().unwrap();
            if s.serpent_gambit_exit_ticks > 0 {
                s.serpent_gambit_exit_ticks -= 1;
            }
            (s.serpent_gambit_exit, s.serpent_gambit_exit_ticks)
        };
        match (exit, left) {
            (Some((x, y)), n) if n > 0 => {
                // Full width while the player is still being bitten at, then
                // shrinking shut over the last stretch — the same `charge` ramp
                // the entry holes open with, run backwards, so opening and
                // closing read as one animation rather than a pop.
                let charge = (n as f32 / SERPENT_GAMBIT_EXIT_CLOSE as f32).clamp(0.0, 1.0);
                draw_rift(c, SERPENT_GAMBIT_EXIT_SLOT, x, y, charge, SERPENT_GAMBIT_HOLE_R);
            }
            _ => {
                if let Some(obj) =
                    c.get_game_object_mut(&format!("serpent_rift_{SERPENT_GAMBIT_EXIT_SLOT}"))
                {
                    obj.visible = false;
                }
            }
        }
    }

    // Seed the holes once, loosely around the player: loose so that moving is
    // still a valid answer, and fixed so the set is readable once placed.
    //
    // Once per ACT, not once per empty list. Capture clears the list, so keying
    // off emptiness alone re-seeded a fresh set of holes around the player on
    // the next frame and took them again while the first bite was still landing.
    // `serpent_gambit_exit` being set is the record that this act already fired.
    {
        let mut s = st.lock().unwrap();
        if s.serpent_rifts.is_empty() && s.serpent_gambit_exit.is_none() {
            for _ in 0..SERPENT_GAMBIT_HOLES {
                let ang = lcg_range(&mut s.seed, 0.0, std::f32::consts::TAU);
                let dist = lcg_range(&mut s.seed, 300.0, SERPENT_GAMBIT_SPREAD);
                s.serpent_rifts.push((
                    player.0 + ang.cos() * dist,
                    player.1 + ang.sin() * dist * 0.7,
                    0,
                ));
            }
        }
    }

    let rifts = { st.lock().unwrap().serpent_rifts.clone() };
    let charge = (ticks as f32 / SERPENT_TELEGRAPH_TICKS as f32).clamp(0.0, 1.0);
    for (i, (x, y, _)) in rifts.iter().enumerate() {
        draw_rift(c, i, *x, *y, charge, SERPENT_GAMBIT_HOLE_R);
    }
    if ticks < SERPENT_TELEGRAPH_TICKS { return; }

    // ── Suction ──────────────────────────────────────────────────────────
    // An open rift PULLS. Waiting to be walked into, the holes were simply
    // scenery: the player could stand still and the attack could not happen,
    // which made the whole act a pause rather than a threat.
    //
    // The pull is applied to the STATE velocity for the same reason the
    // Colossus's well is — the object's momentum write-back would wipe it — and
    // it strengthens toward the mouth, so drifting is survivable and dawdling
    // on the rim is not.
    {
        let mut s = st.lock().unwrap();
        if s.serpent_gambit_react == 0 && !s.dead {
            let mut pull = (0.0_f32, 0.0_f32);
            for (x, y, _) in rifts.iter() {
                let dx = s.px - x;
                let dy = s.py - y;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                if d >= SERPENT_GAMBIT_PULL_R { continue; }
                let t = 1.0 - d / SERPENT_GAMBIT_PULL_R;
                let strength = SERPENT_GAMBIT_PULL * t.powf(0.6);
                pull.0 -= dx / d * strength;
                pull.1 -= dy / d * strength;
            }
            // Clamped as a VECTOR, not per hole. The rifts are seeded within a
            // spread narrower than their own reach, so their fields overlap
            // around the player: summed unclamped, standing between three of
            // them was several times the authored pull and nothing could be
            // done about it. One hole and three holes now pull equally hard —
            // three of them just leave fewer directions that lead out.
            let mag = (pull.0 * pull.0 + pull.1 * pull.1).sqrt();
            if mag > SERPENT_GAMBIT_PULL {
                pull.0 *= SERPENT_GAMBIT_PULL / mag;
                pull.1 *= SERPENT_GAMBIT_PULL / mag;
            }
            s.vx += pull.0;
            s.vy += pull.1;
        }
    }
    // One untether, on the frame the rifts finish charging. Without it a
    // tethered player is simply immune — the rope holds them at a fixed radius
    // and no amount of suction moves them — so the act would be answered by
    // holding still on a hook, which is the opposite of what the fight wants.
    // Re-tethering is both the escape from the drift and the escape from the
    // bite, so the attack asks for the same verb twice rather than two answers.
    if ticks == SERPENT_TELEGRAPH_TICKS {
        let unhooked = {
            let mut s = st.lock().unwrap();
            let was = s.hooked;
            s.hooked = false;
            s.active_hook = String::new();
            was
        };
        if unhooked {
            c.run(Action::Hide { target: Target::name("rope") });
        }
    }

    // Capture: entering a hole delivers the player in front of the head.
    let captured = {
        let s = st.lock().unwrap();
        if s.serpent_gambit_react > 0 { false } else {
            rifts.iter().any(|(x, y, _)| {
                let r = SERPENT_GAMBIT_HOLE_R + PLAYER_R;
                (player.0 - x).powi(2) + (player.1 - y).powi(2) < r * r
            })
        }
    };
    if captured {
        let heading = { st.lock().unwrap().boss_phase };
        let drop = (
            head.0 + heading.cos() * SERPENT_GAMBIT_DROP,
            head.1 + heading.sin() * SERPENT_GAMBIT_DROP,
        );
        {
            let mut s = st.lock().unwrap();
            s.px = drop.0;
            s.py = drop.1;
            s.vx = 0.0;
            s.vy = 0.0;
            s.hooked = false;
            s.active_hook = String::new();
            s.serpent_gambit_react = SERPENT_GAMBIT_REACT;
        }
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position = (drop.0 - PLAYER_R, drop.1 - PLAYER_R);
            obj.momentum = (0.0, 0.0);
        }
        crate::scenes::game::helpers::center_warp_on_player(c);
        // Every entry hole closes the moment one of them lands. Left open they
        // were re-seeded around the player on the very next frame and took them
        // again mid-bite; and the set having done its job is the read — the
        // trap sprung, so it is gone.
        serpent_clear_rifts(c, st);
        // Then the ONE portal the player came out of, set after the clear
        // because the clear wipes it too. Inert: it is not in `serpent_rifts`,
        // so nothing pulls and nothing captures. It is there so the player
        // arrives out of something rather than appearing in mid-air.
        {
            let mut s = st.lock().unwrap();
            s.serpent_gambit_exit = Some(drop);
            s.serpent_gambit_exit_ticks = SERPENT_GAMBIT_REACT + SERPENT_GAMBIT_EXIT_CLOSE;
        }
        return;
    }

    // The reaction window. Tethering inside it is the escape — that is the
    // whole fairness of the attack, so it is checked here and not at the bite.
    let resolve = {
        let mut s = st.lock().unwrap();
        if s.serpent_gambit_react == 0 { None } else {
            s.serpent_gambit_react -= 1;
            if s.hooked {
                s.serpent_gambit_react = 0;
                Some(false) // escaped
            } else if s.serpent_gambit_react == 0 {
                Some(true)  // bitten
            } else {
                None
            }
        }
    };
    if resolve == Some(true) {
        let buffed = { st.lock().unwrap().player_buff > 0 };
        // The bite at the end of the gambit.
        serpent_strike_player(c, st, head, buffed, serpent_cut_for(Strike::Bite));
    }
}
