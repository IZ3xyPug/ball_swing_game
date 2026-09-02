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

/// Where a part sits in the CHAIN, which is not its index in `boss_parts`.
///
/// `boss_parts` is ordered by DESTRUCTION dependency — segments, then tail, then
/// head — because the shared loop unshields part `i` once everything before it
/// is dead. The chain runs head, segments, tail. Conflating the two puts the
/// head at the back of its own body.
pub(crate) fn serpent_chain_index(part_index: usize, part_id: &str) -> usize {
    match part_id {
        "head" => 0,
        "tail" => SERPENT_SEGMENTS + 1,
        _ => part_index + 1,
    }
}

/// Is this segment inside the energised band this frame?
///
/// The band is a moving run of segments, not a per-segment blink — see
/// `SERPENT_SHIELD_BAND`. Distance is measured cyclically along the body so the
/// band wraps from tail back to head without a seam.
pub(crate) fn serpent_shielded_now(chain: usize, band: f32) -> bool {
    let span = (SERPENT_SEGMENTS + 2) as f32;
    let d = (chain as f32 - band).rem_euclid(span);
    let d = d.min(span - d);
    d < SERPENT_SHIELD_BAND * 0.5
}

/// How lit a segment's seam is, 0 armoured to 1 fully exposed. The inverse of
/// the shield, eased so the plating visibly opens rather than popping.
pub(crate) fn serpent_seam(chain: usize, band: f32) -> f32 {
    let span = (SERPENT_SEGMENTS + 2) as f32;
    let d = (chain as f32 - band).rem_euclid(span);
    let d = d.min(span - d);
    let half = SERPENT_SHIELD_BAND * 0.5;
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

        // THE HEAD IS IN THE HOLE — it does not move at all.
        //
        // Steering it AT the portal is not the same as stopping it there: it
        // swam straight through, kept going, turned around because the portal
        // was still its goal, and came back — entering twice per cycle with a
        // loop in between. Nothing about a goal makes a thing stop.
        //
        // With the head frozen, the body still slides in, because `Swallow`
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
    let band = {
        let mut s = st.lock().unwrap();
        s.serpent_band = (s.serpent_band + SERPENT_SHIELD_SPEED / 60.0)
            .rem_euclid((SERPENT_SEGMENTS + 2) as f32);
        s.serpent_band
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
    struct Piece { idx: usize, id: &'static str, pos: (f32, f32), rot: f32,
                   size: f32, chain: usize, shielded: bool, seam: f32, open: bool }
    let pieces: Vec<Piece> = {
        let s = st.lock().unwrap();
        let mut out = Vec::new();
        for (i, p) in s.boss_parts.iter().enumerate() {
            if !p.alive { continue; }
            let chain = serpent_chain_index(i, p.id);
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
            // Inside the active rift: not on screen, and not hittable in
            // either direction. One test covers the whole burrow — diving, a
            // piece enters the radius and vanishes; emerging, it leaves and
            // reappears — so the body goes in head-first one piece at a time
            // and comes back out the same way with no per-piece animation.
            if let Some(hole) = s.serpent_hole {
                let d2 = (pos.0 - hole.0).powi(2) + (pos.1 - hole.1).powi(2);
                if d2 < SERPENT_RIFT_SWALLOW_R * SERPENT_RIFT_SWALLOW_R {
                    continue;
                }
            }
            let (size, id) = match p.id {
                "head" => (SERPENT_HEAD_SIZE, "head"),
                "tail" => (SERPENT_TAIL_SIZE, "tail"),
                _ => (SERPENT_SEGMENT_SIZE, "seg"),
            };
            // A shielded PART (phase gating) is never open. The travelling band
            // is a second, independent gate on top of that.
            let banded = p.id == "seg" && serpent_shielded_now(chain, band);
            out.push(Piece {
                idx: i, id, pos, rot, size, chain,
                shielded: p.shielded || banded,
                seam: if p.shielded { 0.0 } else { serpent_seam(chain, band) },
                // Hittable only once the plating has actually opened. The glow
                // below reads this SAME flag, so the cue and the hit box can
                // never disagree — keyed to the raw band boolean, the glow lit
                // while the armour was still visibly shut.
                open: !p.shielded && serpent_seam(chain, band) >= SERPENT_OPEN_AT,
            });
        }
        out
    };

    for piece in &pieces {
        let name = format!("serpent_part_{}", piece.idx);
        let half = piece.size * 0.5;
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
                shape: ShapeType::Ellipse(0.0, (piece.size, piece.size), piece.rot),
                image: img, color: None,
            });
            obj.size = (piece.size, piece.size);
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
                (piece.size * 1.12, piece.size * 1.12),
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
            let r = piece.size * SERPENT_CONTACT_R + PLAYER_R;
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
                let r = piece.size * SERPENT_CONTACT_R + PLAYER_R;
                (px - piece.pos.0).powi(2) + (py - piece.pos.1).powi(2) < r * r
            }).map(|piece| piece.pos)
        }
    };
    if let Some(at) = contact {
        serpent_strike_player(c, st, at, buffed);
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

/// Throw the player off the body, and cost them a heart if they are unprotected.
fn serpent_strike_player(c: &mut Canvas, st: &Arc<Mutex<State>>, at: (f32, f32), buffed: bool) {
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
        s.hooked = false;
        s.active_hook = String::new();
    }
    c.run(Action::Hide { target: Target::name("rope") });
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
    st.lock().unwrap().serpent_rifts.clear();
    for i in 0..SERPENT_RIFT_COUNT.max(SERPENT_GAMBIT_HOLES as u32) {
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
pub(crate) fn serpent_body_arc() -> f32 {
    (SERPENT_SEGMENTS + 1) as f32 * SERPENT_SEGMENT_SPACING + SERPENT_RIFT_SWALLOW_R
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
            // The head is pinned in the hole and the trail window slides
            // forward, pulling each piece into it in turn.
            RiftPhase::Swallow => {
                s.serpent_arc += SERPENT_RIFT_SWALLOW_SPEED;
                let swallowed = s.serpent_rift_ticks as f32 * SERPENT_RIFT_SWALLOW_SPEED;
                if swallowed >= serpent_body_arc() {
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
                if s.serpent_arc >= serpent_body_arc() {
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
            serpent_strike_player(c, st, hole, buffed);
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
            .map(|(i, p)| (serpent_chain_index(i, p.id), i, p.id))
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
        serpent_strike_player(c, st, (player.0, player.1 + 400.0), buffed);
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
        serpent_strike_player(c, st, pos, buffed);
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
            let chain = serpent_chain_index(i, id);
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
        serpent_strike_player(c, st, at, buffed);
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
    // Seed the holes once, loosely around the player: loose so that moving is
    // still a valid answer, and fixed so the set is readable once placed.
    {
        let mut s = st.lock().unwrap();
        if s.serpent_rifts.is_empty() {
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
        serpent_clear_rifts(c, st);
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
        serpent_strike_player(c, st, head, buffed);
    }
}
