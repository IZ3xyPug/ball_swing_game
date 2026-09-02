// sim_tests.rs — generation + collision invariants.
//
// NOTE: this module used `use super::*` alone, which resolves to lib.rs and
// re-exports nothing the tests need, so the whole file failed to compile and
// every guard below (including the hook-reach guard) was silently dead.
// Import the modules explicitly instead.

use crate::constants::*;
use crate::state::{lcg, lcg_range, gen_hook_batch};
use crate::level_gen::{clamp_into_envelope, hop_dy_budget, hop_is_reachable, starter_hooks};
use crate::difficulty::*;
use crate::objects::{circle_hits_aabb, circle_hits_obb};
use crate::images::{pad_image_cached, spinner_image_cached, flip_image_cached, gate_top_image_cached};
use std::time::Instant;

#[test]
fn gate_gap_is_rare_enough() {
    let mut seed = 0xA11CE_u64;
    let n = 10_000;
    let mut sum = 0.0_f32;
    let mut min_gap = f32::MAX;
    let mut max_gap = f32::MIN;

    for _ in 0..n {
        let g = lcg_range(&mut seed, GATE_GAP_MIN, GATE_GAP_MAX);
        sum += g;
        min_gap = min_gap.min(g);
        max_gap = max_gap.max(g);
    }

    let avg = sum / n as f32;
    assert!(min_gap >= GATE_GAP_MIN);
    assert!(max_gap <= GATE_GAP_MAX);
    // Guardrail for "a lot more rare" while still catching accidental regressions.
    assert!(GATE_GAP_MIN >= 7_000.0, "GATE_GAP_MIN regressed: {GATE_GAP_MIN}");
    assert!(GATE_GAP_MAX >= 11_000.0, "GATE_GAP_MAX regressed: {GATE_GAP_MAX}");
    assert!(avg >= GATE_GAP_MIN + 900.0, "avg gate gap too low: {avg}");
}

#[test]
fn gate_cluster_count_stays_in_1_to_3() {
    let mut seed = 0xBEEFu64;
    for _ in 0..20_000 {
        let count = 1 + ((lcg(&mut seed) * 3.0) as usize);
        assert!((1..=3).contains(&count), "invalid cluster size: {count}");
    }
}

#[test]
fn hook_generation_stays_reachable_and_bounded() {
    // Sample the whole curve, not just one point on it. The pre-fix generator
    // was reachable near distance 0 and produced 806 px hops (vs a 720 px rope)
    // once the difficulty bonuses had ramped in, so a single-distance test
    // could sit green over a broken late game.
    for step in 0..=20 {
        let distance = DIFFICULTY_FULL_DISTANCE * (step as f32 / 20.0);
        let mut seed = 42_u64 ^ (step as u64).wrapping_mul(0x9E37_79B9);
        let mut head_x = SPAWN_X + 800.0;
        let mut head_y = (HOOK_Y_MIN + HOOK_Y_MAX) * 0.5;
        let batch = gen_hook_batch(&mut seed, SPAWN_X + 800.0, &mut head_x, &mut head_y, distance);

        assert_eq!(batch.len(), MAX_HOOKS_LIVE);

        let mut prev = None::<(f32, f32)>;
        for hook in &batch {
            assert!(
                (HOOK_Y_MIN..=HOOK_Y_MAX).contains(&hook.y),
                "hook y out of bounds at d={distance:.0}: {}",
                hook.y
            );
            if let Some((px, py)) = prev {
                assert!(hook.x > px, "hook x must increase");
                let (dx, dy) = (hook.x - px, hook.y - py);
                assert!(
                    hop_is_reachable(dx, dy),
                    "hop outside envelope at d={distance:.0}: dx={dx:.1} dy={dy:.1}"
                );
                let dist = (dx * dx + dy * dy).sqrt();
                assert!(
                    dist >= HOOK_MIN_REACH * 0.6,
                    "hooks too close at d={distance:.0}: {dist:.1} px"
                );
            }
            prev = Some((hook.x, hook.y));
        }
    }
}

#[test]
fn hop_envelope_narrows_to_zero_at_full_stride() {
    // The budget must actually close as dx approaches the horizontal reach —
    // the old clamp was `.min((HOOK_MAX_REACH * HOOK_MAX_REACH).sqrt())`, which
    // is just HOOK_MAX_REACH, so it never bound anything.
    let (up0, down0) = hop_dy_budget(0.0);
    assert!((up0 - HOP_REACH_UP).abs() < 0.01);
    assert!((down0 - HOP_REACH_DOWN).abs() < 0.01);

    let (up_full, down_full) = hop_dy_budget(HOP_REACH_X);
    assert!(up_full < 0.01 && down_full < 0.01, "{up_full} {down_full}");

    // Monotonic, and down is always the more generous direction.
    let mut last = f32::MAX;
    for i in 0..=100 {
        let dx = HOP_REACH_X * (i as f32 / 100.0);
        let (up, down) = hop_dy_budget(dx);
        assert!(down >= up, "down budget must not be tighter than up at dx={dx}");
        assert!(up <= last + 0.01, "budget must not grow with dx");
        last = up;
    }

    assert!(!hop_is_reachable(HOP_REACH_X + 1.0, 0.0), "past horizontal reach");
    assert!(!hop_is_reachable(0.0, -(HOP_REACH_UP + 1.0)), "past upward reach");
    assert!(hop_is_reachable(0.0, HOP_REACH_DOWN - 1.0), "within downward reach");
}

#[test]
fn envelope_clamp_only_pulls_toward_the_previous_node() {
    // The spawner's backstop must never push a node further from its
    // predecessor, only closer — otherwise it could undo a hazard dodge and
    // still hand back an unreachable position.
    let (prev_x, prev_y) = (0.0_f32, 100.0_f32);
    let mut seed = 0xC0FFEE_u64;
    for _ in 0..5_000 {
        let x = lcg_range(&mut seed, 50.0, HOP_REACH_X);
        let y = lcg_range(&mut seed, -1500.0, 1500.0);
        let clamped = clamp_into_envelope(prev_x, prev_y, x, y);
        assert!(
            (clamped - prev_y).abs() <= (y - prev_y).abs() + 0.01,
            "clamp moved the node away from prev: {y} -> {clamped}"
        );
        assert!(
            hop_is_reachable(x - prev_x, clamped - prev_y),
            "clamped hop still unreachable: dx={} dy={}",
            x - prev_x,
            clamped - prev_y
        );
    }
}

#[test]
fn starter_layout_is_reachable() {
    // The first thing a player ever sees used to be the least reachable layout
    // in the game: 1250 px between nodes and two of them below HOOK_Y_MAX.
    let hooks = starter_hooks();
    for pair in hooks.windows(2) {
        let (ax, ay) = pair[0];
        let (bx, by) = pair[1];
        assert!(bx > ax, "starter hooks must advance");
        assert!(
            hop_is_reachable(bx - ax, by - ay),
            "starter hop outside envelope: dx={:.0} dy={:.0}",
            bx - ax,
            by - ay
        );
    }
    for (_, y) in hooks {
        assert!(
            (HOOK_Y_MIN..=HOOK_Y_MAX).contains(&y),
            "starter hook outside the playable band: {y}"
        );
    }
}

#[test]
fn difficulty_curve_spans_the_intended_hour() {
    // Guards the thing that was actually wrong: the ramp used to finish in
    // 30_000 px, which a player crosses in well under a minute.
    assert_eq!(difficulty_t(0.0), 0.0);
    assert_eq!(difficulty_t(DIFFICULTY_GRACE_DISTANCE), 0.0);
    assert!(difficulty_t(DIFFICULTY_FULL_DISTANCE) > 0.999);

    // Monotonic non-decreasing across the whole run.
    let mut prev = 0.0_f32;
    for i in 0..=2_000 {
        let d = DIFFICULTY_FULL_DISTANCE * 1.2 * (i as f32 / 2_000.0);
        let t = difficulty_t(d);
        assert!(t >= prev - 1e-6, "curve went backwards at {d}");
        assert!((0.0..=1.0).contains(&t));
        prev = t;
    }

    // Half-difficulty should land near the middle of the run, not in the first
    // few seconds of it.
    let half = difficulty_t(DIFFICULTY_FULL_DISTANCE * 0.5);
    assert!((0.35..0.65).contains(&half), "curve is lopsided: {half}");

    // A minute in, the game should still be close to the easy end.
    let one_minute = difficulty_t(DIFFICULTY_PX_PER_MINUTE);
    assert!(one_minute < 0.05, "first minute is already hard: {one_minute}");
}

#[test]
fn zones_cycle_instead_of_saturating() {
    // `zone_index_for_distance` used to be `.min(2)`, so the backdrop stopped
    // changing ~50 s into a run and never changed again.
    assert_eq!(zone_index_for_distance(0.0), 0);
    assert_eq!(zone_index_for_distance(ZONE_CYCLE_DISTANCE * 1.5), 1);
    assert_eq!(zone_index_for_distance(ZONE_CYCLE_DISTANCE * 2.5), 2);
    assert_eq!(zone_index_for_distance(ZONE_CYCLE_DISTANCE * 3.5), 0);

    let mut seen = [false; ZONE_COUNT];
    for i in 0..40 {
        seen[zone_index_for_distance(ZONE_CYCLE_DISTANCE * (i as f32 + 0.5))] = true;
    }
    assert!(seen.iter().all(|v| *v), "not every zone is reachable");

    // Over a full hour the backdrop should turn over a healthy number of times.
    let laps = (DIFFICULTY_FULL_DISTANCE / ZONE_CYCLE_DISTANCE) as usize;
    assert!(laps >= 9, "only {laps} zone changes in a full run");
}

#[test]
fn circle_aabb_collision_pushes_out() {
    // Circle overlapping a 100x100 rect at origin from the left side.
    let push = circle_hits_aabb((10.0, 50.0), 25.0, (0.0, 0.0), (100.0, 100.0));
    assert!(push.is_some());
    let (px, _py) = push.unwrap();
    assert!(px < 0.0, "expected leftward push, got {px}");
}

#[test]
fn circle_obb_collision_detects_rotated_hit() {
    // Rotated spinner-like rectangle around (300, 300).
    let push = circle_hits_obb(
        (300.0, 300.0),
        40.0,
        (300.0 - SPINNER_W * 0.5, 300.0 - SPINNER_H * 0.5),
        (SPINNER_W, SPINNER_H),
        32.0,
    );
    assert!(push.is_some(), "expected collision with rotated OBB");
}

#[test]
fn cached_images_are_reused() {
    let p1 = pad_image_cached();
    let p2 = pad_image_cached();
    assert!(std::sync::Arc::ptr_eq(&p1, &p2));

    let s1 = spinner_image_cached();
    let s2 = spinner_image_cached();
    assert!(std::sync::Arc::ptr_eq(&s1, &s2));

    let f1 = flip_image_cached();
    let f2 = flip_image_cached();
    assert!(std::sync::Arc::ptr_eq(&f1, &f2));

    let g1 = gate_top_image_cached();
    let g2 = gate_top_image_cached();
    assert!(std::sync::Arc::ptr_eq(&g1, &g2));
}

#[test]
fn startup_cache_smoke_budget() {
    // Loose smoke budget: catches accidental expensive per-call regeneration.
    let start = Instant::now();
    for _ in 0..50_000 {
        let _ = pad_image_cached();
        let _ = spinner_image_cached();
        let _ = flip_image_cached();
        let _ = gate_top_image_cached();
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_secs_f32() < 2.5, "cache smoke too slow: {elapsed:?}");
}

// ── Permanent upgrades (the meta loop) ───────────────────────────────────────

#[test]
fn every_permanent_upgrade_is_buyable_and_terminates() {
    use crate::profile::{PERM_UPGRADES, upgrade_cost};

    for u in PERM_UPGRADES {
        assert!(u.max >= 1, "{} has no ranks", u.id);
        assert!(u.growth > 1.0, "{} cost does not grow", u.id);
        assert!(!u.blurb.is_empty(), "{} has no blurb", u.id);

        // Cost must rise with every rank and stop exactly at max, so an
        // upgrade can always be finished and never becomes free.
        let mut prev = 0u64;
        for owned in 0..u.max {
            let cost = upgrade_cost(u, owned)
                .unwrap_or_else(|| panic!("{} rank {owned} has no price", u.id));
            assert!(cost > prev, "{} rank {owned} is not dearer than the last", u.id);
            prev = cost;
        }
        assert!(
            upgrade_cost(u, u.max).is_none(),
            "{} is still purchasable past its max",
            u.id
        );
    }
}

#[test]
fn permanent_upgrade_ids_are_unique_and_stable() {
    use crate::profile::{PERM_UPGRADES, upgrade_by_id};
    // Ids are written into save files, so a duplicate would silently merge two
    // upgrades' ranks and a lookup miss would drop a player's purchases.
    for (i, a) in PERM_UPGRADES.iter().enumerate() {
        assert!(upgrade_by_id(a.id).is_some(), "{} is not resolvable by id", a.id);
        for b in &PERM_UPGRADES[i + 1..] {
            assert_ne!(a.id, b.id, "duplicate upgrade id {}", a.id);
            assert_ne!(a.name, b.name, "duplicate upgrade name {}", a.name);
        }
    }
}

#[test]
fn full_meta_loop_is_reachable_within_a_sane_grind() {
    use crate::profile::{PERM_UPGRADES, upgrade_cost};
    // Total cost of maxing everything, against what a run pays out. This is a
    // balance guard, not a correctness one: it fails loudly if a cost curve is
    // edited into something that would take thousands of runs.
    let total: u64 = PERM_UPGRADES
        .iter()
        .map(|u| (0..u.max).filter_map(|o| upgrade_cost(u, o)).sum::<u64>())
        .sum();

    // A decent run yields roughly one boss reward plus distance meta.
    let per_run = META_BOSS_REWARD.max(1) * 3;
    let runs = total / per_run;
    assert!(
        (20..=400).contains(&runs),
        "maxing everything costs {total} meta = ~{runs} runs, which is outside the intended 20..400"
    );
}

#[test]
fn permanent_bonuses_default_to_neutral() {
    // A profile with nothing bought must not change the game at all — every
    // multiplier is applied unconditionally in the hot path.
    let b = crate::profile::PermBonuses::default();
    assert_eq!(b.extra_hearts, 0);
    assert_eq!(b.reach_mult, 1.0);
    assert_eq!(b.momentum_mult, 1.0);
    assert_eq!(b.magnet_mult, 1.0);
    assert_eq!(b.start_coins, 0);
    assert_eq!(b.flare_wards, 0);
    assert_eq!(b.free_respawns, 0);
}

// ── Game modes and boss pacing ───────────────────────────────────────────────

#[test]
fn normal_mode_schedules_bosses_across_the_hour() {
    use crate::mode::*;
    let n = boss_count(GameMode::Normal);
    // Enough fights that each is an event, few enough that swinging stays the
    // bulk of the run. Guards a retune of BOSS_INTERVAL_MINUTES that would
    // quietly turn the mode into a boss rush or a boss drought.
    assert_eq!(n, BOSS_ROSTER_SIZE, "Normal must run the whole roster, once each");

    // Evenly spread: no fight should sit much closer to its neighbour than the
    // average. The previous schedule crammed the seventh fight four minutes
    // before the finale.
    let step = DIFFICULTY_FULL_DISTANCE / BOSS_ROSTER_SIZE as f32;

    // Strictly increasing, and the last one lands at the top of the curve.
    let mut prev = 0.0_f32;
    for i in 0..n {
        let d = boss_trigger_distance(GameMode::Normal, i).unwrap();
        assert!(d > prev, "fight {i} is not past fight {}", i as i32 - 1);
        assert!(
            (d - prev - step).abs() < step * 0.05,
            "fight {i} is {:.0} px after the last, not the even {step:.0}",
            d - prev
        );
        prev = d;
    }
    assert!(
        (prev - DIFFICULTY_FULL_DISTANCE).abs() < 1.0,
        "the final fight is at {prev}, not at the top of the curve ({DIFFICULTY_FULL_DISTANCE})"
    );
    assert!(is_final_boss(GameMode::Normal, n - 1));
    assert!(!is_final_boss(GameMode::Normal, 0));
    assert_eq!(boss_trigger_distance(GameMode::Normal, n), None);
}

#[test]
fn casual_mode_never_schedules_a_boss() {
    use crate::mode::*;
    assert_eq!(boss_count(GameMode::Casual), 0);
    for i in 0..8 {
        assert_eq!(boss_trigger_distance(GameMode::Casual, i), None);
    }
    assert!(!GameMode::Casual.has_bosses());
    assert!(!GameMode::Casual.has_ending());
}

#[test]
fn boss_rush_is_short_links_between_every_fight() {
    use crate::mode::*;
    let n = boss_count(GameMode::BossRush);
    assert_eq!(n, BOSS_ROSTER_SIZE);
    // Every link is the same short hop, and all of them together are a small
    // fraction of a Normal run — the mode is the fights, not the level.
    let mut prev = 0.0_f32;
    for i in 0..n {
        let d = boss_trigger_distance(GameMode::BossRush, i).unwrap();
        assert!((d - prev - BOSS_RUSH_LINK_DISTANCE).abs() < 1.0);
        prev = d;
    }
    assert!(prev < DIFFICULTY_FULL_DISTANCE * 0.05, "rush links total {prev}, too long");
    assert!(!GameMode::BossRush.allows_space_zone());
}

#[test]
fn mode_indices_round_trip() {
    use crate::mode::*;
    // Indices are persisted in saves and canvas vars, so a renumber would
    // silently reinterpret every stored record.
    for m in [GameMode::Casual, GameMode::Normal, GameMode::BossRush] {
        assert_eq!(GameMode::from_index(m.index()), m);
    }
    assert_eq!(GameMode::Casual.index(), 0);
    assert_eq!(GameMode::Normal.index(), 1);
    assert_eq!(GameMode::BossRush.index(), 2);
    // Out-of-range falls back to the core mode rather than panicking.
    assert_eq!(GameMode::from_index(99), GameMode::Normal);
    assert_eq!(GameMode::from_index(-1), GameMode::Normal);
}

#[test]
fn boss_arenas_never_overlap_the_level() {
    use crate::mode::*;
    // Arenas are stepped along X far past anything the generator reaches, and
    // the player is warped in. If a later arena could land where the level is,
    // a fight would drop the player into live world geometry.
    let arena_w = BOSS_ZONE_X2 - BOSS_ZONE_X1;
    let stride = arena_w + BOSS_ARENA_GAP;
    let furthest = BOSS_ARENA_ORIGIN_X + stride * (BOSS_ROSTER_SIZE.max(boss_count(GameMode::Normal)) as f32);
    assert!(
        BOSS_ARENA_ORIGIN_X > DIFFICULTY_FULL_DISTANCE + SPAWN_X + GEN_AHEAD,
        "the first arena sits inside the reachable level"
    );
    assert!(stride > arena_w, "arenas would overlap each other");
    assert!(furthest.is_finite());
}

#[test]
fn hazard_density_tightens_but_never_collapses() {
    use crate::difficulty::*;
    // Every hazard gap in the game was a fixed constant, so density in minute
    // 58 matched minute 2. Gaps must shrink with the curve — and must never
    // shrink to nothing, or late game becomes impassable rather than dense.
    for (lo, hi) in [
        (PAD_GAP_MIN, PAD_GAP_MAX),
        (SPINNER_GAP_MIN, SPINNER_GAP_MAX),
        (GWELL_GAP_MIN, GWELL_GAP_MAX),
        (TURRET_GAP_MIN, TURRET_GAP_MAX),
    ] {
        let (elo, ehi) = hazard_gap_range(0.0, lo, hi);
        assert!((elo - lo).abs() < 1.0, "easy end must be the authored value");
        assert!((ehi - hi).abs() < 1.0);

        let (hlo, hhi) = hazard_gap_range(DIFFICULTY_FULL_DISTANCE, lo, hi);
        assert!(hlo < elo && hhi < ehi, "gap did not tighten with the curve");
        assert!(hlo > 0.0 && hlo >= lo * 0.4, "gap collapsed to {hlo}");
        assert!(hlo <= hhi, "min/max ordering inverted");
    }
    assert!(
        (0.3..1.0).contains(&HAZARD_GAP_HARD_SCALE),
        "HAZARD_GAP_HARD_SCALE {HAZARD_GAP_HARD_SCALE} is outside a sane range"
    );
}

#[test]
fn boss_schedule_reads_as_intended() {
    use crate::mode::*;
    // Named minutes, so the pacing is legible in the test output and a retune
    // that changes the felt rhythm of a run fails loudly rather than silently.
    let mins: Vec<i32> = (0..boss_count(GameMode::Normal))
        .map(|i| (boss_trigger_distance(GameMode::Normal, i).unwrap()
            / DIFFICULTY_PX_PER_MINUTE).round() as i32)
        .collect();
    assert_eq!(mins, vec![11, 23, 34, 46, 57, 69, 80], "Normal boss minutes changed");
    assert!(
        (BOSS_INTERVAL_MINUTES - DIFFICULTY_FULL_MINUTES / BOSS_ROSTER_SIZE as f32).abs() < 0.01,
        "the advertised interval no longer matches the schedule"
    );

    // The pacing check that can be verified in-game rather than on paper.
    // Nominal minutes assume a fixed player speed; DISTANCE is what the world
    // actually measures, and god-mode straight-line flight is how it was timed.
    // Playtest 2026-08-27 put god-mode at ~980 px/s and asked for 3–4 minutes
    // per gap, which is 176 000–235 000 px.
    let gap = boss_trigger_distance(GameMode::Normal, 0).unwrap();
    assert!(
        (176_000.0..=235_000.0).contains(&gap),
        "boss gap is {gap:.0} px = {:.1} min of god-mode flight; playtest asked for 3-4",
        gap / 980.0 / 60.0
    );
}

// ── Hazard introduction schedule ─────────────────────────────────────────────

#[test]
fn hazards_are_introduced_one_at_a_time_in_order() {
    use crate::hazards::*;
    // The authored order. A reorder here changes what a player learns first and
    // must be a deliberate edit, not a side effect.
    let expected = [
        Hazard::Spinner,
        Hazard::GravityWell,
        Hazard::Turret,
        Hazard::DriftAsteroid,
        Hazard::Comet,
        Hazard::SolarFlare,
    ];
    let actual: Vec<Hazard> = STAGES.iter().map(|s| s.hazard).collect();
    assert_eq!(actual, expected, "hazard introduction order changed");

    // Each gets several minutes to itself before the next arrives, so it is
    // learned in isolation rather than in a pile.
    let mut prev = 0.0_f32;
    for s in STAGES {
        assert!(
            s.introduce_minutes() - prev >= 5.0,
            "{} arrives only {:.1} min after the previous hazard",
            s.name,
            s.introduce_minutes() - prev
        );
        assert!(
            s.mature_minutes() > s.introduce_minutes() + 8.0,
            "{} matures too fast to be learned",
            s.name
        );
        assert!(
            s.mature_minutes() <= DIFFICULTY_FULL_MINUTES,
            "{} never reaches maturity inside a run",
            s.name
        );
        prev = s.introduce_minutes();
    }
}

#[test]
fn the_opening_is_nodes_pads_and_asteroids_only() {
    use crate::hazards::*;
    // The first minutes must contain no hazards at all — the player is learning
    // to swing, and everything that can hurt them arrives later.
    let opening = 4.0 * DIFFICULTY_PX_PER_MINUTE;
    for s in STAGES {
        assert!(
            !hazard_active(opening, s.hazard),
            "{} is already live in the opening",
            s.name
        );
    }
    // And the supports are at full strength there.
    for c in SUPPORTS {
        assert_eq!(support_level(0.0, c.support), 1.0, "{} does not start full", c.name);
        assert_eq!(support_level(opening, c.support), 1.0, "{} thins during the opening", c.name);
    }
}

#[test]
fn every_hazard_is_live_and_mature_by_the_end() {
    use crate::hazards::*;
    let end = DIFFICULTY_FULL_DISTANCE;
    for s in STAGES {
        assert!(hazard_active(end, s.hazard), "{} never appears", s.name);
        assert!(
            hazard_intensity(end, s.hazard) > 0.98,
            "{} is still ramping at the end of a run",
            s.name
        );
    }
}

#[test]
fn hazard_intensity_is_monotonic_and_bounded() {
    use crate::hazards::*;
    for s in STAGES {
        let mut prev = -1.0_f32;
        for i in 0..=600 {
            let d = DIFFICULTY_FULL_DISTANCE * 1.1 * (i as f32 / 600.0);
            let t = hazard_intensity(d, s.hazard);
            assert!((0.0..=1.0).contains(&t), "{} intensity {t} out of range", s.name);
            assert!(t >= prev - 1e-6, "{} intensity went backwards at {d}", s.name);
            prev = t;
        }
        // It must arrive at its RAREST, not at its authored density.
        let at_intro = s.introduce_distance();
        assert!(hazard_intensity(at_intro, s.hazard) < 0.01, "{} arrives already dangerous", s.name);
    }
}

#[test]
fn supports_thin_but_never_disappear() {
    use crate::hazards::*;
    for c in SUPPORTS {
        let mut prev = 2.0_f32;
        for i in 0..=600 {
            let d = DIFFICULTY_FULL_DISTANCE * 1.1 * (i as f32 / 600.0);
            let lvl = support_level(d, c.support);
            assert!(lvl <= prev + 1e-6, "{} grew instead of thinning at {d}", c.name);
            assert!(lvl >= c.floor - 1e-6, "{} fell through its floor: {lvl}", c.name);
            assert!(lvl > 0.0, "{} disappeared entirely", c.name);
            prev = lvl;
        }
        assert!(
            (0.3..0.8).contains(&c.floor),
            "{} floor {} is outside a sane range — too low is a wall, too high is no curve",
            c.name, c.floor
        );
        // Gaps widen as the support thins, and stay finite.
        let (elo, _) = support_gap_range(0.0, c.support, 1000.0, 2000.0);
        let (hlo, hhi) = support_gap_range(DIFFICULTY_FULL_DISTANCE, c.support, 1000.0, 2000.0);
        assert!(hlo > elo, "{} gaps did not widen", c.name);
        // The widest a gap can get is the authored value divided by the floor.
        assert!(
            hhi.is_finite() && hhi <= 2000.0 / c.floor + 1.0,
            "{} gaps widened past their floor: {hhi}",
            c.name
        );
    }
}

#[test]
fn hazards_get_more_dangerous_but_stay_survivable() {
    use crate::hazards::*;
    let end = DIFFICULTY_FULL_DISTANCE;
    let intro_t = |h: Hazard| stage(h).introduce_distance();

    // Turrets fire more often — and never faster than twice a second, which is
    // the point where a single turret becomes a wall rather than a hazard.
    let slow = turret_shoot_interval(intro_t(Hazard::Turret), TURRET_SHOOT_INTERVAL_FAST);
    let fast = turret_shoot_interval(end, TURRET_SHOOT_INTERVAL_FAST);
    assert!(fast < slow, "turret fire rate did not increase");
    assert!(fast >= 30, "turrets fire faster than twice a second: {fast} ticks");

    // Comets arrive in bursts, growing one at a time rather than jumping.
    assert_eq!(comet_burst_count(intro_t(Hazard::Comet)), 1, "comets start in bursts");
    assert_eq!(comet_burst_count(end), 3, "comet bursts never reach three");
    let mut prev = 1;
    for i in 0..=400 {
        let d = end * (i as f32 / 400.0);
        let n = comet_burst_count(d);
        assert!(n >= prev && n - prev <= 1, "comet burst jumped {prev} -> {n}");
        assert!((1..=3).contains(&n));
        prev = n;
    }
    assert!(comet_interval(end, COMET_SPAWN_INTERVAL) < comet_interval(intro_t(Hazard::Comet), COMET_SPAWN_INTERVAL));
    assert!(comet_fire_chance(end) > comet_fire_chance(intro_t(Hazard::Comet)));

    // Asteroids only enter the play area after their introduction, and never
    // take over the spawn entirely — the high ones are still a support.
    assert_eq!(drift_asteroid_share(0.0), 0.0);
    assert_eq!(drift_asteroid_share(intro_t(Hazard::DriftAsteroid)), 0.0);
    let share = drift_asteroid_share(end);
    assert!((0.2..0.6).contains(&share), "drift share {share} leaves too few high asteroids");

    // Density rises for every gap-scaled hazard, and no gap collapses.
    for h in [Hazard::Spinner, Hazard::GravityWell, Hazard::Turret] {
        let (rlo, _) = hazard_gap_range(intro_t(h), h, 7000.0, 11000.0, 2.4, 0.55);
        let (dlo, _) = hazard_gap_range(end, h, 7000.0, 11000.0, 2.4, 0.55);
        assert!(dlo < rlo, "{:?} did not get denser", h);
        assert!(dlo > 1500.0, "{:?} gap collapsed to {dlo}", h);
    }
}

// ── Chain-frontier ownership ─────────────────────────────────────────────────

#[test]
fn only_sanctioned_code_raises_the_hook_chain_frontier() {
    // `rightmost_x` gates world generation: the chain spawner stops while
    // `rightmost_x >= px + GEN_AHEAD`. Any other system that RAISES it switches
    // generation off until the player walks the difference.
    //
    // That is not a hypothetical — `spawn_upgrade_nodes` raised it to a
    // companion node placed up to 55 000 px ahead, which killed hook generation
    // from the first second of every run and left the whole stretch to the
    // `ensure_player_hooks` failsafe. Five call sites borrow from the shared
    // hook pool, so this is a source-level guard against a sixth.
    //
    // Writes that LOWER the frontier (`.min(...)`, respawn and boss backfill)
    // are always safe and are not counted.
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();
    let mut sanctioned = 0usize;

    // (file suffix, how many raising writes that file is allowed)
    const ALLOWED: &[(&str, usize)] = &[
        // The chain spawner itself, plus the frontier-repair clamp beside it.
        ("scenes/game/spawning.rs", 2),
        // The respawn checkpoint node, which is always BEHIND the player and so
        // cannot open a gap ahead of them.
        ("scenes/game/hearts.rs", 1),
        // Run construction, plus the `--at-minute` test hook that teleports the
        // whole world state forward. That one sets `gen_head_x` to the same
        // value in the same breath, so `rightmost_x <= gen_head_x` still holds
        // and the chain resumes cleanly from the seeded position.
        ("scenes/game/build_scene.rs", 1),
    ];

    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let rel = path
                .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // sim_tests.rs is this file; it only mentions the field in prose.
            if rel == "sim_tests.rs" {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (n, line) in text.lines().enumerate() {
                let t = line.trim();
                if t.starts_with("//") || !t.contains("rightmost_x") {
                    continue;
                }
                // A write to the FIELD (`x.rightmost_x = ...`), not a local
                // binding of the same name and not a lowering clamp. Struct
                // literal initialisation is construction, not a write.
                let is_field_write = t.contains(".rightmost_x =") && !t.contains("==");
                if !is_field_write || t.contains(".min(") {
                    continue;
                }
                let allowed = ALLOWED.iter().find(|(f, _)| rel.ends_with(f));
                match allowed {
                    Some(_) => sanctioned += 1,
                    None => offenders.push(format!("{rel}:{} — {t}", n + 1)),
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these raise the hook chain frontier and will stall world generation:\n  {}\n\
         An auxiliary hook may borrow from the pool, but must not advance the chain.",
        offenders.join("\n  ")
    );

    let budget: usize = ALLOWED.iter().map(|(_, n)| *n).sum();
    assert!(
        sanctioned <= budget,
        "{sanctioned} raising writes in sanctioned files, budget is {budget} — \
         a new one was added; confirm it is the chain spawner before widening this"
    );
}

// ── Boss approach: the solar eclipse ─────────────────────────────────────────

#[test]
fn eclipse_darkens_then_lifts_before_the_teleporter() {
    use crate::scenes::game::eclipse_curve;

    // Nothing before the approach begins.
    let (_, far) = eclipse_curve(BOSS_ECLIPSE_RANGE);
    assert_eq!(far, 0.0, "the eclipse starts before its own range");
    let (_, beyond) = eclipse_curve(BOSS_ECLIPSE_RANGE * 2.0);
    assert_eq!(beyond, 0.0);

    // Peak dark exactly at the release point, not at the teleporter.
    let (_, peak) = eclipse_curve(BOSS_ECLIPSE_RELEASE);
    assert!(peak > 0.99, "the eclipse never reaches full darkness: {peak}");

    // Fully lifted by the time the player arrives, so the black hole is lit at
    // the moment it matters.
    let (_, at_gate) = eclipse_curve(0.0);
    assert!(at_gate < 0.01, "still dark at the teleporter: {at_gate}");

    // Single-peaked: rises monotonically to the release point, falls after.
    let mut prev = -1.0_f32;
    let mut gap = BOSS_ECLIPSE_RANGE;
    while gap >= BOSS_ECLIPSE_RELEASE {
        let (_, d) = eclipse_curve(gap);
        assert!(d >= prev - 1e-5, "darkness dipped while approaching at gap {gap}");
        prev = d;
        gap -= 250.0;
    }
    let mut prev = 2.0_f32;
    let mut gap = BOSS_ECLIPSE_RELEASE;
    while gap >= 0.0 {
        let (_, d) = eclipse_curve(gap);
        assert!(d <= prev + 1e-5, "darkness rose again during the release at gap {gap}");
        prev = d;
        gap -= 100.0;
    }

    // The warning has to land before the world reacts to it.
    let (rise_at_start, dark_at_start) = eclipse_curve(BOSS_ECLIPSE_RANGE - 1.0);
    assert!(rise_at_start < ECLIPSE_WARN_FRACTION, "no warning window");
    assert!(dark_at_start < 0.02, "the world darkens before the warning");

    // The release must be a real beat, not a flicker.
    assert!(
        BOSS_ECLIPSE_RELEASE >= 2_000.0 && BOSS_ECLIPSE_RELEASE < BOSS_ECLIPSE_RANGE * 0.4,
        "release window of {BOSS_ECLIPSE_RELEASE} px is not a legible beat"
    );
}

#[test]
fn the_boss_approach_is_long_enough_to_read() {
    use crate::mode::*;
    // The teleport used to arrive with no build-up, because the marker was
    // pinned at a constant while the trigger became scheduled. Both the eclipse
    // and the marker must start well before the threshold, and the eclipse
    // must come first so the sequence reads warning -> dark -> black hole.
    assert!(
        BOSS_ECLIPSE_RANGE > BOSS_APPROACH_RANGE,
        "the black hole appears before the eclipse that announces it"
    );
    assert!(
        BOSS_APPROACH_RANGE > BOSS_ECLIPSE_RELEASE,
        "the marker appears after the dark has already lifted"
    );
    // And the whole approach must fit inside one boss gap.
    let gap = boss_trigger_distance(GameMode::Normal, 0).unwrap();
    assert!(
        BOSS_ECLIPSE_RANGE < gap * 0.5,
        "the approach ({BOSS_ECLIPSE_RANGE} px) eats more than half the {gap:.0} px run-up"
    );
}

#[test]
fn the_eclipse_lights_the_whole_trail_with_a_chain() {
    // One lamp cannot light a trail evenly: `atten` falls off from a single
    // origin, so the trail always dims out behind the player however wide the
    // lamp is. Lights spaced back along it keep it lit along its length — the
    // approach taken by the build this was recovered from.
    assert!(
        ECLIPSE_TRAIL_LIGHTS.len() >= 3,
        "too few trail lights to cover its length"
    );
    let furthest = ECLIPSE_TRAIL_LIGHTS
        .iter()
        .map(|(off, _, _)| off.abs())
        .fold(0.0_f32, f32::max);
    assert!(furthest >= 150.0, "the chain does not reach back along the trail");

    // Intensity falls off along the chain, so the light reads as coming FROM
    // the player rather than as a row of separate lamps.
    let mut prev = f32::MAX;
    for (off, _, intensity) in ECLIPSE_TRAIL_LIGHTS.iter().filter(|(o, _, _)| o.abs() >= 50.0) {
        let _ = off;
        assert!(*intensity <= prev + 0.01, "trail light brightens with distance");
        prev = *intensity;
    }
    // Only the main lamp casts shadows, so they stay defined rather than
    // smeared by several origins.
    assert!(
        ECLIPSE_PLAYER_LIGHT_INTENSITY > prev,
        "the main lamp is not the brightest source on the player"
    );

    // The night-mode post pass is what makes any of this visible on dark art:
    // quartz lighting is multiplicative, so bloom is the only thing that can
    // spread light beyond the sprites it lands on. Its threshold must sit below
    // what the lamp produces, or nothing blooms at all.
    assert!(
        ECLIPSE_BLOOM_THRESHOLD < LIGHT_NDL_2D * ECLIPSE_PLAYER_LIGHT_INTENSITY,
        "the bloom threshold is above anything the lamp can produce"
    );
    assert!(ECLIPSE_BLOOM_STRENGTH > 0.0, "bloom disabled — the pool will be invisible");
    assert!(ECLIPSE_VIGNETTE_STRENGTH > 0.0, "no vignette — the frame edges will not darken");
}

#[test]
fn every_node_and_well_slot_has_its_own_light() {
    // A shared pool repositioned onto the nearest few has to re-rank as the
    // player moves, and every re-rank switches lights on and off — nodes
    // appearing to light and go dark as you pass them. One light per POOL SLOT,
    // attached to the object, has stable identity and never pops.
    //
    // `LightingConfig::default()` caps at 64 while the GPU uniform and the
    // shader loop both hold 256, and `emit_lights` silently `take`s the first
    // `max_lights` out of a HashMap — so exceeding the cap drops an arbitrary
    // subset rather than failing.
    const GPU_MAX_LIGHTS: usize = 256;
    let eclipse = HOOK_POOL_SIZE + ECLIPSE_GWELL_LIGHT_COUNT + 1 /* player lamp */;
    assert!(
        LIGHTING_MAX_LIGHTS >= eclipse,
        "capacity {LIGHTING_MAX_LIGHTS} cannot hold the eclipse's {eclipse} lights"
    );
    // Headroom for the boss fight's own lights on top.
    assert!(
        LIGHTING_MAX_LIGHTS - eclipse >= 25,
        "no headroom left for the boss fight's lights"
    );
    assert!(LIGHTING_MAX_LIGHTS <= GPU_MAX_LIGHTS, "capacity exceeds the GPU uniform");

    // Markers restore their own sprite exactly, and no further.
    assert!(
        (ECLIPSE_NODE_LIGHT_INTENSITY * LIGHT_NDL_2D - 1.0).abs() < 0.01,
        "node markers do not restore to exactly normal brightness"
    );
    assert!(ECLIPSE_NODE_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.5, "markers reach too far");
}

#[test]
fn the_eclipse_reaches_darkness_before_it_is_over() {
    use crate::scenes::game::eclipse_curve;
    // The warning window must still exist, but the dark has to arrive well
    // before the release or the approach spends most of its length doing
    // nothing visible.
    let at = |gap: f32| eclipse_curve(gap).1;

    // Halfway through the darkening ramp, the ambient should already be most of
    // the way down.
    let mid_gap = BOSS_ECLIPSE_RELEASE + (BOSS_ECLIPSE_RANGE - BOSS_ECLIPSE_RELEASE) * 0.5;
    let d = at(mid_gap);
    let fall = ((d - ECLIPSE_WARN_FRACTION) / (1.0 - ECLIPSE_WARN_FRACTION)).clamp(0.0, 1.0);
    let inv = 1.0 - fall;
    let eased = 1.0 - inv * inv;
    assert!(
        eased > 0.6,
        "only {eased:.2} dark at the midpoint — the eclipse takes too long to arrive"
    );

    // And the warning still lands before anything changes.
    assert!(ECLIPSE_WARN_FRACTION > 0.05, "no warning window at all");
    assert!(ECLIPSE_WARN_FRACTION < 0.25, "the warning holds full light too long");
}

// ── Gravity cannon under flipped gravity ─────────────────────────────────────

#[test]
fn the_cannon_fires_forward_in_both_gravity_orientations() {
    // Flipped gravity draws the world upside down. The barrel angle mirrors
    // about the horizontal axis; it does NOT rotate a half-turn, which would
    // negate the horizontal component too and fire the player back down the
    // level. It also has to SWEEP the mirrored way while charging, or it winds
    // up visibly backwards before firing backwards.
    fn launch(rotation_deg: f32, flipped: bool) -> (f32, f32) {
        let base = if flipped { -rotation_deg } else { rotation_deg };
        let r = base.to_radians();
        let vx = CANNON_LAUNCH_VX * r.cos() - CANNON_LAUNCH_VY * r.sin();
        let vy = CANNON_LAUNCH_VX * r.sin() + CANNON_LAUNCH_VY * r.cos();
        (vx, if flipped { -vy } else { vy })
    }
    fn dir(flipped: bool) -> f32 { if flipped { -1.0 } else { 1.0 } }

    for flipped in [false, true] {
        let rest = if flipped {
            CANNON_DEFAULT_ROTATION + 180.0
        } else {
            CANNON_DEFAULT_ROTATION
        };
        // The barrel ends its charge sweep here.
        let fired_at = rest + CANNON_CHARGE_ROTATION_DEG * dir(flipped);
        let (vx, vy) = launch(fired_at, flipped);

        assert!(vx > 0.0, "cannon fires backwards when flipped={flipped}: vx={vx:.0}");
        assert!(
            vx > CANNON_LAUNCH_VX * 0.4,
            "cannon barely moves the player forward when flipped={flipped}: vx={vx:.0}"
        );
        // "Up" is -Y normally and +Y in a vertically mirrored world; either way
        // the launch must carry the player AWAY from the floor they stand on.
        let up = if flipped { vy } else { -vy };
        assert!(up > 0.0, "cannon fires into the floor when flipped={flipped}: vy={vy:.0}");
    }

    // Both orientations must produce the same shot, just mirrored.
    let un = launch(CANNON_DEFAULT_ROTATION + CANNON_CHARGE_ROTATION_DEG, false);
    let fl = launch(
        (CANNON_DEFAULT_ROTATION + 180.0) - CANNON_CHARGE_ROTATION_DEG,
        true,
    );
    assert!((un.0 - fl.0).abs() < 0.01, "forward speed differs between orientations");
    assert!((un.1 + fl.1).abs() < 0.01, "vertical speed is not a clean mirror");
}

#[test]
fn the_eclipse_goes_fully_dark_partway_through() {
    use crate::scenes::game::eclipse_curve;
    // Outside the player's lamp the world has to actually go dark, and get
    // there partway rather than only at the very end — the first pass held the
    // ambient at 0.14, which was bright enough to play by and made the lamp
    // decorative.
    let ambient_at = |gap: f32| {
        let dark = eclipse_curve(gap).1;
        let fall = ((dark - ECLIPSE_WARN_FRACTION) / (1.0 - ECLIPSE_WARN_FRACTION)).clamp(0.0, 1.0);
        let fall = (fall / ECLIPSE_FULL_DARK_AT).clamp(0.0, 1.0);
        let inv = 1.0 - fall;
        let eased = 1.0 - inv * inv;
        1.0 + (ECLIPSE_MIN_AMBIENT - 1.0) * eased
    };

    // Halfway between the start of the approach and the release point the world
    // must be visibly dark. The exact floor depends on whether the player's lamp
    // is doing any work: with `ECLIPSE_USE_POINT_LIGHTS` off there is nothing to
    // see by, so the floor has to stay playable.
    let mid = BOSS_ECLIPSE_RELEASE + (BOSS_ECLIPSE_RANGE - BOSS_ECLIPSE_RELEASE) * 0.5;
    assert!(
        ambient_at(mid) < 0.12,
        "still {:.2} ambient at the midpoint — not dark enough to need the lamp",
        ambient_at(mid)
    );
    // And it holds there rather than continuing to creep.
    assert!(ambient_at(BOSS_ECLIPSE_RELEASE) <= ambient_at(mid) + 0.01);
    // Never pitch black: the danger floor has to stay findable.
    assert!(ECLIPSE_MIN_AMBIENT > 0.0, "full black hides the death floor");
    // Without a working lamp the floor must stay playable; with one it can go
    // much lower, because the lamp is then what the player sees by.
    // Multiplicative, so this IS the fraction of authored brightness an unlit
    // sprite keeps — low enough to read as an eclipse, high enough to leave
    // silhouettes and the danger floor findable.
    // Matches `AmbientLight::dark()`, which is what `LightingConfig::night()`
    // uses in the build this was taken from.
    assert!(
        (0.02..0.12).contains(&ECLIPSE_MIN_AMBIENT),
        "floor ambient {ECLIPSE_MIN_AMBIENT} no longer matches the night preset"
    );
    // Full light before the warning has landed.
    assert!(ambient_at(BOSS_ECLIPSE_RANGE) > 0.99);
}

#[test]
fn the_boss_buff_is_a_window_not_a_licence() {
    // Ten seconds of weakpoint damage AND three absorbed hits meant one node
    // carried most of a fight. The buff should be long enough to convert an
    // opening and short enough that it has to be re-earned.
    assert!(
        (180..=360).contains(&BUFF_DURATION_TICKS),
        "buff lasts {BUFF_DURATION_TICKS} ticks ({:.1} s), outside the intended 3-6 s",
        BUFF_DURATION_TICKS as f32 / 60.0
    );
    assert!(BUFF_ABSORB_MAX >= 1 && BUFF_ABSORB_MAX <= 4);
    // It must not outlast the gap between chances to re-earn it.
    assert!(
        (BUFF_DURATION_TICKS as f32 / 60.0) < 8.0,
        "the buff covers most of a fight on its own"
    );
}

#[test]
fn shadow_casters_stay_under_the_renderer_limit() {
    // wgpu_canvas uploads at most `MAX_OCCLUDERS` (32) and silently DROPS the
    // rest, and quartz collects them in object-store order rather than by
    // distance — so exceeding the cap does not degrade gracefully, it makes an
    // arbitrary subset cast. That is what made spinners throw shadows while
    // pads, flagged in the same pass, did not.
    const RENDERER_MAX_OCCLUDERS: usize = 32;
    assert!(
        ECLIPSE_MAX_SHADOW_CASTERS < RENDERER_MAX_OCCLUDERS,
        "the eclipse alone flags {ECLIPSE_MAX_SHADOW_CASTERS} occluders against a hard cap of \
         {RENDERER_MAX_OCCLUDERS}"
    );
    // Headroom for anything else in the scene that sets `shadow_caster`.
    assert!(
        RENDERER_MAX_OCCLUDERS - ECLIPSE_MAX_SHADOW_CASTERS >= 8,
        "no headroom left for other shadow casters"
    );
    // Enough to actually populate a scene.
    assert!(ECLIPSE_MAX_SHADOW_CASTERS >= 12, "too few casters to read as a lit world");
}

#[test]
fn markers_are_separated_by_reach_not_by_brightness() {
    // Every light in the eclipse restores its subject to exactly normal
    // brightness — none of them may exceed it, because `lit` clamps and going
    // past 1.0 washes a sprite toward white. So what distinguishes a marker
    // from the lamp is RADIUS, not intensity.
    // Markers restore their own sprite; only the lamp is driven past that, so
    // that the bloom pass has something to pick up.
    for (name, i) in [
        ("node marker", ECLIPSE_NODE_LIGHT_INTENSITY),
        ("well marker", ECLIPSE_GWELL_LIGHT_INTENSITY),
    ] {
        assert!(
            (i * LIGHT_NDL_2D - 1.0).abs() < 0.02,
            "{name} restores to {:.2} of normal, not 1.0",
            i * LIGHT_NDL_2D
        );
    }
    assert!(
        ECLIPSE_PLAYER_LIGHT_INTENSITY * LIGHT_NDL_2D > 1.0,
        "the lamp never crosses the bloom threshold"
    );
    // Markers illuminate themselves; only the lamp lights an area.
    // Markers illuminate their own sprite; only the lamp lights an area.
    assert!(ECLIPSE_NODE_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.5);
    assert!(ECLIPSE_GWELL_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.5);
}

// ── The Colossus torso's two-attack rhythm ───────────────────────────────────

#[test]
fn torso_alternates_slam_and_storm() {
    // Strict alternation, not a random pick. Two storms in a row would be a
    // long stretch with no window to damage the torso at all; two slams wastes
    // the contrast the pair exists to create.
    let seq: Vec<TorsoAttack> = (1..=8).map(torso_attack_for).collect();
    for pair in seq.windows(2) {
        assert_ne!(pair[0], pair[1], "two identical torso attacks in a row: {seq:?}");
    }
    let vents = seq.iter().filter(|a| **a == TorsoAttack::CoreVent).count();
    assert_eq!(vents, seq.len() / 2, "the vulnerability-opening attack must be half the cycle");
}

#[test]
fn meteor_storm_is_a_sequence_from_alternating_sides() {
    let mut seed = 0xC0FFEE_u64;
    let queue = crate::scenes::game::boss::meteor_storm_schedule(&mut seed);

    assert_eq!(queue.len(), COLOSSUS_METEOR_COUNT as usize);

    // SEQUENTIAL: every launch is 0.5-1.0 s after the one before it. A burst
    // that lands together is one coin flip; a sequence is a series of dodges.
    let delays: Vec<u32> = queue.iter().map(|(d, _)| *d).collect();
    for w in delays.windows(2) {
        let gap = w[1] - w[0];
        assert!(
            gap >= COLOSSUS_METEOR_GAP_MIN && gap <= COLOSSUS_METEOR_GAP_MAX,
            "meteor gap {gap} ticks outside {COLOSSUS_METEOR_GAP_MIN}..{COLOSSUS_METEOR_GAP_MAX}"
        );
    }
    assert!(delays.windows(2).all(|w| w[1] > w[0]), "launch times must be strictly increasing");

    // Every meteor must finish launching inside the attack it belongs to,
    // otherwise the storm ends with rocks still queued.
    assert!(
        *delays.last().unwrap() < COLOSSUS_STORM_TICKS,
        "last meteor launches at {} but the storm ends at {COLOSSUS_STORM_TICKS}",
        delays.last().unwrap()
    );

    // ALTERNATING SIDES: consecutive meteors come from opposite halves of the
    // sky, so the player is moved rather than allowed to settle in one corner.
    let mid = (COLOSSUS_METEOR_ANGLE_MIN + COLOSSUS_METEOR_ANGLE_MAX) * 0.5;
    let sides: Vec<bool> = queue.iter().map(|(_, a)| *a >= mid).collect();
    for w in sides.windows(2) {
        assert_ne!(w[0], w[1], "two consecutive meteors from the same side: {sides:?}");
    }

    // Never from below — a meteor rising off the floor reads as a bug.
    for (_, angle) in &queue {
        assert!(
            *angle >= COLOSSUS_METEOR_ANGLE_MIN && *angle <= COLOSSUS_METEOR_ANGLE_MAX,
            "meteor angle {angle} outside the above-and-to-the-sides band"
        );
    }
}

// ── The head's gaze beam ─────────────────────────────────────────────────────

#[test]
fn beam_is_a_fixed_length_ray_past_the_player() {
    // The beam used to stop AT the player, which made standing beyond the aim
    // point unconditionally safe.
    let head = (0.0, 0.0);
    for aim in [(300.0, 0.0), (-120.0, 80.0), (0.0, 1500.0)] {
        let end = crate::scenes::game::boss::beam_end(head, aim);
        let len = (end.0 * end.0 + end.1 * end.1).sqrt();
        assert!(
            (len - COLOSSUS_BEAM_LENGTH).abs() < 1.0,
            "beam to {aim:?} is {len:.0} px, not {COLOSSUS_BEAM_LENGTH}"
        );
        // and it still points through the aim point
        let d = (aim.0 * aim.0 + aim.1 * aim.1).sqrt().max(1.0);
        let dot = (end.0 / len) * (aim.0 / d) + (end.1 / len) * (aim.1 / d);
        assert!(dot > 0.999, "beam to {aim:?} does not run through the aim point");
    }
}

#[test]
fn a_straight_beam_and_a_curved_one_use_the_same_path() {
    use crate::scenes::game::boss::beam_point as bp;
    let a = (0.0, 0.0);
    let b = (1000.0, 0.0);

    // curve 0 collapses to the straight line, so there is one code path rather
    // than a straight case and a curved case that can disagree.
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let p = bp(a, b, 0.0, t);
        assert!((p.0 - t * 1000.0).abs() < 0.01 && p.1.abs() < 0.01, "curve 0 bent at t={t}");
    }

    // A curved beam bows to one side and still starts and ends where it says.
    let curve = COLOSSUS_BEAM_CURVE_MAX;
    assert!((bp(a, b, curve, 0.0).0 - a.0).abs() < 0.01);
    assert!((bp(a, b, curve, 1.0).0 - b.0).abs() < 0.01);
    let mid = bp(a, b, curve, 0.5);
    // Control point sits `curve * len` off the chord, so the curve itself
    // reaches half that.
    let expected_bow = curve * 1000.0 * 0.5;
    assert!(
        (mid.1.abs() - expected_bow).abs() < 1.0,
        "curved beam bows {:.0} px at its midpoint, expected {:.0}",
        mid.1.abs(), expected_bow
    );
    assert!(mid.1.abs() > 100.0, "the bow has to be big enough to have to be read");
}

#[test]
fn beam_damage_area_matches_what_is_drawn() {
    // The old test was `point_segment_dist < PATH_THICKNESS + PLAYER_R` against
    // a strip drawn PATH_THICKNESS tall — a hit box twice as wide as the art.
    // The damaging half-width is now half the drawn thickness plus the player.
    let hit_r = crate::scenes::game::boss::beam_hit_radius();
    assert!(
        (hit_r - (COLOSSUS_BEAM_THICKNESS * 0.5 + PLAYER_R)).abs() < 0.01,
        "hit radius {hit_r} does not match the drawn beam"
    );
    // and it is genuinely wider than the beam it replaces
    assert!(
        hit_r > COLOSSUS_PATH_THICKNESS + PLAYER_R,
        "the reworked beam must damage a WIDER area than the one it replaces"
    );
}

#[test]
fn a_gaze_attack_is_two_or_three_beams_back_to_back() {
    let shot = COLOSSUS_BEAM_TICKS + COLOSSUS_BEAM_GAP_TICKS;
    assert_eq!(COLOSSUS_BEAM_GAP_TICKS, 30, "the gap between beams should be ~0.5 s");
    assert!(COLOSSUS_BEAM_SHOTS_MIN >= 2 && COLOSSUS_BEAM_SHOTS_MAX <= 3);

    // The burst has to fit inside the attack, and the attack has to be long
    // enough that the longest burst is not cut off.
    for shots in COLOSSUS_BEAM_SHOTS_MIN..=COLOSSUS_BEAM_SHOTS_MAX {
        let duration = shots * shot;
        assert!(duration >= 2 * shot);
        // every beam gets its full sweep
        for n in 0..shots {
            assert!(n * shot + COLOSSUS_BEAM_TICKS <= duration);
        }
    }
}

#[test]
fn the_head_rearms_about_a_second_after_its_window_closes() {
    // The window has to end cleanly rather than being cut short by the next
    // gravity well opening on top of it.
    let idle = crate::scenes::game::boss::colossus_idle_len(3);
    assert_eq!(idle, COLOSSUS_HEAD_VULN_AFTER + COLOSSUS_HEAD_REARM_GAP);
    let gap = idle - COLOSSUS_HEAD_VULN_AFTER;
    assert!((45..=90).contains(&gap), "re-arm gap is {gap} ticks, wanted ~1 s");
    // and the window itself is a second or two longer than the other parts'
    assert!(COLOSSUS_HEAD_VULN_AFTER >= COLOSSUS_VULN_AFTER_TICKS + 120);
}

// ── Arena dressing ───────────────────────────────────────────────────────────

#[test]
fn boss_arenas_are_mostly_small_and_medium_asteroids() {
    let sizes: Vec<f32> = (0..BOSS_ASTEROID_COUNT).map(boss_arena_asteroid_size).collect();
    let large = sizes.iter().filter(|s| **s >= BOSS_ASTEROID_MEDIUM_MAX).count();
    let small = sizes.iter().filter(|s| **s < BOSS_ASTEROID_SMALL_MAX).count();

    assert!(
        large * 8 <= sizes.len(),
        "{large}/{} arena asteroids are large; big ones block sight lines to the fight",
        sizes.len()
    );
    assert!(
        small * 2 >= sizes.len(),
        "only {small}/{} are small; the arena needs things to swing around, not walls",
        sizes.len()
    );
    for s in &sizes {
        assert!(
            *s >= SPACE_ASTEROID_SIZE_MIN && *s <= SPACE_ASTEROID_SIZE_MAX,
            "asteroid size {s} outside the authored band"
        );
    }
}

// ── Night mode ───────────────────────────────────────────────────────────────

#[test]
fn the_darkness_attack_and_the_eclipse_share_one_look() {
    // The attack used to drop the ambient and nothing else, which under a
    // multiplicative lighting model is a blank screen rather than a dark room.
    // Both events now bottom out at the same ambient and differ only in framing.
    assert!((BOSS_DARK_AMBIENT - ECLIPSE_MIN_AMBIENT).abs() < 0.001);

    // The attack closes in tighter and faster than the slow approach does.
    assert!(BOSS_DARK_VIGNETTE_RADIUS < ECLIPSE_VIGNETTE_RADIUS);
    assert!(BOSS_DARK_VIGNETTE_STRENGTH > ECLIPSE_VIGNETTE_STRENGTH);

    // Bloom is what makes the lamp visible at all on dark art, so both presets
    // must sit below what the lamp actually produces.
    for (name, threshold) in [
        ("eclipse", ECLIPSE_BLOOM_THRESHOLD),
        ("boss darkness", BOSS_DARK_BLOOM_THRESHOLD),
    ] {
        assert!(
            threshold < ECLIPSE_PLAYER_LIGHT_INTENSITY * LIGHT_NDL_2D,
            "{name} bloom threshold {threshold} is above what the lamp reaches, so nothing blooms"
        );
    }
}

#[test]
fn eclipse_bloom_was_dialled_back() {
    // Tuned down after play: at 1.2 / 0.30 the spread swallowed the shapes it
    // was supposed to reveal. Threshold decides WHAT blooms and strength decides
    // HOW FAR, so both move — dropping strength alone dims without sharpening.
    assert!(ECLIPSE_BLOOM_STRENGTH < 1.2, "bloom strength was not reduced");
    assert!(ECLIPSE_BLOOM_THRESHOLD > 0.30, "fewer pixels should qualify as bright");
    // but not so far that the effect stops working
    assert!(ECLIPSE_BLOOM_STRENGTH > 0.4);
}

// ── The core vent: dangerous and vulnerable at the same time ─────────────────

/// Is the torso hittable `t` ticks into a given FSM state during a core vent?
/// Mirrors the rule in `tick_multi_part_boss` so the timing can be asserted
/// without standing up a whole fight.
fn vent_open(state: &str, t: u32) -> bool {
    match state {
        "attack" => t >= COLOSSUS_VENT_VULN_DELAY,
        "recover" => t < COLOSSUS_VENT_VULN_AFTER,
        // The after-window outlasts the recovery, so it carries on into the
        // idle. Counted from the end of the vent, not from the start of a state.
        "idle" => COLOSSUS_RECOVER_TICKS + t < COLOSSUS_VENT_VULN_AFTER,
        _ => false,
    }
}

#[test]
fn the_vent_window_opens_during_the_attack_not_after_it() {
    // The whole point of this attack: unlike the hands and the head, which give
    // a safe window once a danger has passed, the torso's window is open WHILE
    // the spokes are still turning. The counter-attack has to be taken under
    // fire.
    assert!(!vent_open("attack", 0), "the wind-up must not be free");
    assert!(!vent_open("attack", COLOSSUS_VENT_VULN_DELAY - 1));
    assert!(vent_open("attack", COLOSSUS_VENT_VULN_DELAY), "window opens ~0.5s in");
    assert!(vent_open("attack", COLOSSUS_VENT_TICKS - 1), "and stays open to the end of the vent");

    // ~0.5 s of wind-up before it opens.
    assert!((25..=40).contains(&COLOSSUS_VENT_VULN_DELAY), "wind-up should be about half a second");

    // Most of the vent is a window, or the attack is not worth approaching.
    let open_ticks = COLOSSUS_VENT_TICKS - COLOSSUS_VENT_VULN_DELAY;
    assert!(
        open_ticks * 2 > COLOSSUS_VENT_TICKS,
        "only {open_ticks}/{COLOSSUS_VENT_TICKS} ticks of the vent are hittable"
    );
}

#[test]
fn the_vent_window_outlasts_the_danger_by_a_couple_of_seconds() {
    // The vent ends with the player thrown clear and untethered. A one-second
    // window was mostly spent swinging back, so the counter-attack rarely
    // landed — and this is the torso's ONLY hittable beat, so a window that is
    // hard to reach makes the torso effectively immortal.
    assert!(vent_open("recover", 0), "still hittable the moment the spokes cut out");
    assert!(
        (120..=210).contains(&COLOSSUS_VENT_VULN_AFTER),
        "after-window is {COLOSSUS_VENT_VULN_AFTER} ticks; wanted 2-3.5 s"
    );

    // It has to actually survive the recovery and reach into the idle, or the
    // extra length would be silently capped at the recovery's length.
    assert!(
        COLOSSUS_VENT_VULN_AFTER > COLOSSUS_RECOVER_TICKS,
        "the window ends inside the recovery, so lengthening it changed nothing"
    );
    assert!(vent_open("recover", COLOSSUS_RECOVER_TICKS - 1), "open through the whole recovery");
    assert!(vent_open("idle", 0), "and on into the lull after it");

    // ...but it must still close, and close well before the torso acts again.
    let idle_open = COLOSSUS_VENT_VULN_AFTER - COLOSSUS_RECOVER_TICKS;
    assert!(!vent_open("idle", idle_open), "window must close");
    let next_attack_at = crate::scenes::game::boss::colossus_idle_len(2);
    assert!(
        idle_open < next_attack_at,
        "the window ({idle_open} into the idle) runs into the next attack (at {next_attack_at})"
    );
}

#[test]
fn the_spokes_sweep_the_whole_circle_a_couple_of_times() {
    // Four spokes 90 degrees apart, so 90 degrees of rotation sweeps every
    // angle once. The vent should be a couple of passes to weave through, not
    // an endless chase.
    let swept = COLOSSUS_VENT_TICKS as f32 * COLOSSUS_VENT_SPIN;
    let per_sweep = 360.0 / COLOSSUS_VENT_SPOKES as f32;
    let sweeps = swept / per_sweep;
    assert!(
        (1.5..=3.0).contains(&sweeps),
        "the vent sweeps every angle {sweeps:.1} times; wanted about two"
    );
    // and a spoke has to actually reach the player's playing space
    assert!(COLOSSUS_VENT_LENGTH > COLOSSUS_TORSO_ZONE_R * 2.0);
}

// ── The clap ─────────────────────────────────────────────────────────────────

#[test]
fn hands_alternate_lunge_and_clap() {
    let seq: Vec<HandAttack> = (1..=8).map(hand_attack_for).collect();
    for pair in seq.windows(2) {
        assert_ne!(pair[0], pair[1], "two identical hand attacks in a row: {seq:?}");
    }
}

#[test]
fn the_clap_throws_you_whether_or_not_it_connects() {
    // The wave reaches far beyond the hands themselves: being outside the kill
    // zone is not the same as being unaffected.
    assert!(
        COLOSSUS_CLAP_WAVE_R > COLOSSUS_HAND_ZONE_R * 2.0,
        "the wave barely reaches past the hands, so a dodged clap is a non-event"
    );
    // And it throws hard enough to matter — well past the normal speed cap.
    assert!(
        COLOSSUS_CLAP_WAVE_POWER > MOMENTUM_CAP,
        "the clap's throw is inside the normal speed cap, so it would not read as a throw"
    );

    // Falls off to nothing at the edge, so the reach is readable rather than a
    // hard boundary the player cannot see.
    let at_edge = COLOSSUS_CLAP_WAVE_POWER * (1.0 - (COLOSSUS_CLAP_WAVE_R / COLOSSUS_CLAP_WAVE_R));
    assert!(at_edge.abs() < 0.01, "the wave should fade out, not stop dead");
}

#[test]
fn clapped_hands_are_only_hittable_once_they_are_home() {
    // The reward for reading a clap is a clean window at a known place, not a
    // scramble into the middle of the arena while everything else is live.
    // Mirrors the rule in `tick_multi_part_boss`.
    let open = |state: &str, t: u32| match state {
        "idle" => t < COLOSSUS_CLAP_VULN_AFTER,
        _ => false,
    };
    assert!(!open("attack", 999), "not while they are jammed together mid-arena");
    assert!(!open("recover", 0), "not on the way back either");
    assert!(open("idle", 0), "hittable the moment they are home");
    assert!(open("idle", COLOSSUS_CLAP_VULN_AFTER - 1));
    assert!(!open("idle", COLOSSUS_CLAP_VULN_AFTER), "about a second, then closed");
    assert!((45..=90).contains(&COLOSSUS_CLAP_VULN_AFTER));
}

#[test]
fn a_lone_hand_never_claps() {
    // Mirrors the gate in `tick_multi_part_boss`: the attack is chosen from the
    // pair's counter AND from both hands being in the fight. Gated on the
    // counter alone, a surviving hand would keep committing to a clap it cannot
    // complete — and would keep the clap's "only hittable once home" rule while
    // doing it, so destroying one hand would have made the other HARDER to
    // finish off.
    let chosen = |n: u32, both_hands_ready: bool| {
        if both_hands_ready { hand_attack_for(n) } else { HandAttack::Lunge }
    };
    for n in 1..=8 {
        assert_eq!(
            chosen(n, false),
            HandAttack::Lunge,
            "a single surviving hand committed to a clap at n={n}"
        );
    }
    // With both hands present the alternation is untouched.
    assert!((1..=8).any(|n| chosen(n, true) == HandAttack::Clap));
}

// ── Beam cost ────────────────────────────────────────────────────────────────

#[test]
fn a_straight_beam_is_drawn_as_one_quad() {
    use crate::scenes::game::boss::beam_polyline;
    // Half of all beams are straight, and subdividing one into
    // COLOSSUS_BEAM_SEGMENTS produced that many large alpha-blended rectangles
    // where one has the identical shape. The gaze attack's frame cost was fill,
    // not CPU, so this is the difference that matters.
    let a = (0.0, 0.0);
    let b = (COLOSSUS_BEAM_LENGTH, 0.0);

    let straight = beam_polyline(a, b, 0.0, 1.0);
    assert_eq!(straight.len(), 2, "a straight beam should need one segment");
    assert!((straight[1].0 - b.0).abs() < 0.01, "and still reach its endpoint");

    // A partial sweep still ends at the front, not at the full length.
    let half = beam_polyline(a, b, 0.0, 0.5);
    assert_eq!(half.len(), 2);
    assert!((half[1].0 - COLOSSUS_BEAM_LENGTH * 0.5).abs() < 0.01);

    // A curved beam still gets the subdivision it actually needs.
    let curved = beam_polyline(a, b, COLOSSUS_BEAM_CURVE_MAX, 1.0);
    assert_eq!(curved.len(), COLOSSUS_BEAM_SEGMENTS + 1);
}

#[test]
fn the_telegraph_only_draws_what_is_still_ahead() {
    use crate::scenes::game::boss::beam_polyline_range;
    let a = (0.0, 0.0);
    let b = (COLOSSUS_BEAM_LENGTH, 0.0);

    // Before the beam fires, the whole ray is ahead.
    let full = beam_polyline_range(a, b, 0.0, 0.0, 1.0);
    assert!((full[0].0 - 0.0).abs() < 0.01 && (full[1].0 - b.0).abs() < 0.01);

    // Half way through the sweep, only the far half is drawn — the near half is
    // already covered by the bright core, so drawing it again is pure overdraw.
    let ahead = beam_polyline_range(a, b, 0.0, 0.5, 1.0);
    assert_eq!(ahead.len(), 2);
    assert!(
        (ahead[0].0 - COLOSSUS_BEAM_LENGTH * 0.5).abs() < 0.01,
        "the telegraph should start at the beam front, not at the head"
    );
    assert!((ahead[1].0 - b.0).abs() < 0.01, "and still run to the far end");

    // Fully swept: nothing left to telegraph.
    assert!(beam_polyline_range(a, b, 0.0, 1.0, 1.0).is_empty());
    assert!(beam_polyline_range(a, b, 0.0, 0.8, 0.3).is_empty(), "an inverted range draws nothing");

    // A curved beam keeps its subdivision, and still starts at the front.
    let curved = beam_polyline_range(a, b, COLOSSUS_BEAM_CURVE_MAX, 0.5, 1.0);
    assert_eq!(curved.len(), COLOSSUS_BEAM_SEGMENTS + 1);
    assert!(curved[0].0 > COLOSSUS_BEAM_LENGTH * 0.3, "curved telegraph starts at the front too");
}

#[test]
fn beam_explosions_stay_smaller_than_the_beam() {
    // Pops larger than the beam, eight at a time, over an already-translucent
    // beam, is most of a frame's fill spent on decoration.
    assert!(
        COLOSSUS_BEAM_EXPLODE_R1 * 2.0 < COLOSSUS_BEAM_THICKNESS * 2.5,
        "a contact explosion is far wider than the beam it belongs to"
    );
    assert!(COLOSSUS_BEAM_EXPLODE_R0 < COLOSSUS_BEAM_EXPLODE_R1, "pops should grow");
    assert!(
        COLOSSUS_BEAM_EXPLODE_MAX_LIVE <= 4,
        "too many concurrent pops; this is the fill-heaviest part of the attack"
    );
}

// ── Phase gating ─────────────────────────────────────────────────────────────

#[test]
fn a_part_is_not_vulnerable_the_moment_its_shield_drops() {
    // Every post-attack window is "Idle, and fewer than N ticks in". A shielded
    // part is pinned to Idle at tick 0 every frame, so the instant its shield
    // dropped — when the part it depended on was destroyed — it matched that
    // condition exactly and was fully open without ever having attacked. The
    // head died on arrival right after the torso; the torso did the same after
    // the hands. `post_attack` is what separates the two cases.
    let open = |post_attack: bool, state: &str, t: u32, window: u32| match state {
        "idle" => post_attack && t < window,
        "recover" => true,
        _ => false,
    };

    for (name, window) in [
        ("head", COLOSSUS_HEAD_VULN_AFTER),
        ("hand", COLOSSUS_VULN_AFTER_TICKS),
        ("clap", COLOSSUS_CLAP_VULN_AFTER),
    ] {
        // Freshly unshielded: Idle, tick 0, never attacked.
        assert!(
            !open(false, "idle", 0, window),
            "{name} is hittable the instant it becomes active"
        );
        assert!(!open(false, "idle", window / 2, window), "{name} stays shut until it attacks");
        // Having actually finished an attack, the window works as intended.
        assert!(open(true, "idle", 0, window), "{name} should open after its own attack");
        assert!(open(true, "idle", window - 1, window));
        assert!(!open(true, "idle", window, window), "{name} window must still close");
    }

    // The vent's window is counted from the end of the vent, so it spans the
    // recovery — the same gate has to apply to its idle tail.
    let vent_idle = |post_attack: bool, t: u32| {
        post_attack && COLOSSUS_RECOVER_TICKS + t < COLOSSUS_VENT_VULN_AFTER
    };
    assert!(!vent_idle(false, 0), "the torso is hittable the instant both hands die");
    assert!(vent_idle(true, 0), "and open after a real vent");
}

#[test]
fn a_new_part_still_gets_to_attack_before_it_can_be_hurt() {
    // The gate must not make a part permanently invulnerable: it clears when an
    // attack begins and is set when one ends, so the first window arrives one
    // full attack cycle after the shield drops.
    let idle_len = crate::scenes::game::boss::colossus_idle_len(3);
    assert!(idle_len > 0, "a newly active part must still reach its telegraph");
    // The head's cycle: idle -> telegraph -> burst -> recover, then the window.
    let burst = COLOSSUS_BEAM_SHOTS_MIN * (COLOSSUS_BEAM_TICKS + COLOSSUS_BEAM_GAP_TICKS);
    let to_first_window = idle_len + COLOSSUS_TELEGRAPH_TICKS + burst;
    assert!(
        to_first_window > COLOSSUS_PART_INVULN_TICKS,
        "the shield-drop grace should be the FSM's own cycle, not just the kill cooldown"
    );
}

// ── Boss order override (testing) ────────────────────────────────────────────

#[test]
fn the_roster_override_permutes_only_real_bosses() {
    // The menu picks from BOSS_ROSTER, so it can never name a boss that does
    // not exist — and the shipped order must be exactly that list, or the menu
    // shows one thing and the game loads another.
    assert_eq!(BOSS_ROSTER.len(), crate::mode::BOSS_ROSTER_SIZE as usize);
    for (i, kind) in BOSS_ROSTER.iter().enumerate() {
        assert_eq!(
            boss_kind_for_index(i as u32), *kind,
            "BOSS_ROSTER[{i}] disagrees with the shipped boss_kind_for_index"
        );
    }
    // Every roster entry is distinct, or a slot could never be reached.
    for i in 0..BOSS_ROSTER.len() {
        for j in (i + 1)..BOSS_ROSTER.len() {
            assert_ne!(BOSS_ROSTER[i], BOSS_ROSTER[j], "duplicate boss in the roster");
        }
    }
}

#[test]
fn a_short_override_still_finishes_the_run() {
    // Testing usually assigns one or two slots. The rest must fall through to
    // the shipped order rather than repeating the last pick, or a run
    // configured for one fight would never reach an end.
    set_boss_order_override(Some(vec![BossKind::Serpent]));
    assert_eq!(boss_kind_for_index(0), BossKind::Serpent, "slot 1 honours the override");
    assert_eq!(
        boss_kind_for_index(1), BossKind::Conductor,
        "past the override, the shipped order resumes"
    );
    assert!(boss_order_is_overridden());

    // Clearing restores the shipped order exactly.
    set_boss_order_override(None);
    assert!(!boss_order_is_overridden());
    for (i, kind) in BOSS_ROSTER.iter().enumerate() {
        assert_eq!(boss_kind_for_index(i as u32), *kind);
    }
}

// ── Serpent body ─────────────────────────────────────────────────────────────

#[test]
fn the_body_retraces_the_heads_path() {
    use crate::scenes::game::boss::serpent::{serpent_push_trail, serpent_trail_point};
    let mut trail: Vec<(f32, f32, f32)> = Vec::new();
    let mut arc = 0.0_f32;

    // Drive the head along a right-angle turn. A segment one spacing behind
    // must still be on the FIRST leg while the head is on the second — that is
    // the difference between following a path and bobbing on a shared curve.
    for i in 0..400 {
        serpent_push_trail(&mut trail, &mut arc, (i as f32 * 4.0, 0.0));
    }
    let corner = arc;
    for i in 1..200 {
        serpent_push_trail(&mut trail, &mut arc, (1596.0, i as f32 * 4.0));
    }

    let behind = arc - corner + 100.0; // 100px back along the first leg
    let ((x, y), _) = serpent_trail_point(&trail, arc, behind).expect("trail point");
    assert!(y.abs() < 1.0, "a segment past the corner should still be on the flat leg, y={y}");
    assert!((x - 1496.0).abs() < 8.0, "and 100px back from it, x={x}");

    // Spacing is even regardless of how fast the head moved.
    let a = serpent_trail_point(&trail, arc, 0.0).unwrap().0;
    let b = serpent_trail_point(&trail, arc, SERPENT_SEGMENT_SPACING).unwrap().0;
    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    assert!(
        d <= SERPENT_SEGMENT_SPACING + 1.0,
        "segments must never sit further apart than their spacing (got {d})"
    );
}

#[test]
fn the_shield_band_always_leaves_a_way_in() {
    use crate::scenes::game::boss::serpent::{serpent_shielded_now, serpent_seam};
    // The band gates ACCESS; if it ever covered the whole body the fight would
    // stall with nothing hittable, which is the failure the band exists to
    // avoid in the first place.
    for step in 0..240 {
        let band = step as f32 * 0.05;
        let exposed = (1..=SERPENT_SEGMENTS)
            .filter(|&c| !serpent_shielded_now(c, band))
            .count();
        assert!(exposed > 0, "every segment shielded at band {band}");
    }
    assert!(
        SERPENT_SHIELD_BAND < SERPENT_SEGMENTS as f32,
        "the band must be narrower than the body"
    );

    // The seam is the inverse of the shield, so the art can never say
    // "hit me" while the shield is up.
    for c in 1..=SERPENT_SEGMENTS {
        for step in 0..40 {
            let band = step as f32 * 0.3;
            if serpent_shielded_now(c, band) {
                assert!(serpent_seam(c, band) < 0.5, "seam lit under an active shield");
            }
        }
    }
}

#[test]
fn the_chain_order_is_not_the_destruction_order() {
    use crate::scenes::game::boss::serpent::serpent_chain_index;
    // `boss_parts` is ordered segments -> tail -> head so the shared loop
    // unshields in that sequence. The BODY runs head -> segments -> tail.
    // Conflating them puts the head at the back of its own body.
    assert_eq!(serpent_chain_index(9, "head"), 0, "the head leads the chain");
    assert_eq!(serpent_chain_index(8, "tail"), SERPENT_SEGMENTS + 1, "the tail trails it");
    for i in 0..SERPENT_SEGMENTS {
        let chain = serpent_chain_index(i, "seg");
        assert!(chain > 0 && chain <= SERPENT_SEGMENTS, "segment {i} sits between them");
    }

    // And the destruction order really is segments, then tail, then head.
    let parts = boss_parts_for_kind(BossKind::Serpent);
    assert_eq!(parts.len(), SERPENT_SEGMENTS + 2);
    assert!(parts[..SERPENT_SEGMENTS].iter().all(|p| !p.shielded), "segments start exposed");
    assert!(parts[SERPENT_SEGMENTS].shielded, "the tail is shielded until the body is gone");
    assert!(parts[SERPENT_SEGMENTS + 1].shielded, "and the head after it");
    assert_eq!(parts[SERPENT_SEGMENTS].id, "tail");
    assert_eq!(parts[SERPENT_SEGMENTS + 1].id, "head");
}

#[test]
fn the_serpent_is_not_a_longer_grind_than_the_colossus() {
    // Diffuse HP reads as longer than concentrated HP, so the Serpent must not
    // also HAVE more of it — the difficulty is meant to come from the shield
    // band gating access, not from the pile behind it.
    let serpent: i32 = boss_parts_for_kind(BossKind::Serpent).iter().map(|p| p.max_hp).sum();
    let colossus: i32 = boss_parts_for_kind(BossKind::Colossus).iter().map(|p| p.max_hp).sum();
    assert!(
        serpent <= colossus,
        "serpent needs {serpent} buffed hits vs the colossus's {colossus}"
    );
    assert!(serpent >= 25, "but it should still feel like a long body ({serpent})");
}

// ── Serpent acts ─────────────────────────────────────────────────────────────

/// Build a part list with the given survivors, mirroring the real one.
fn serpent_parts_with(segments: usize, tail: bool, head: bool) -> Vec<BossPart> {
    let mut parts = boss_parts_for_kind(BossKind::Serpent);
    let mut seg_seen = 0;
    for p in parts.iter_mut() {
        match p.id {
            "seg" => { seg_seen += 1; if seg_seen > segments { p.alive = false; } }
            "tail" => { p.alive = tail; p.shielded = false; }
            _ => { p.alive = head; p.shielded = false; }
        }
    }
    parts
}

#[test]
fn removing_a_part_removes_its_attack() {
    // Availability is decided by which parts survive, not by a phase counter,
    // so the fight's structure and its anatomy stay the same statement.
    use crate::scenes::game::boss::serpent::serpent_acts;
    // Full body: the body attacks, plus the tail launch.
    let acts = serpent_acts(&serpent_parts_with(SERPENT_SEGMENTS, true, true));
    assert!(acts.contains(&SerpentAct::Coil));
    assert!(acts.contains(&SerpentAct::RiftStrikes));
    assert!(acts.contains(&SerpentAct::TailLaunch));
    assert!(!acts.contains(&SerpentAct::SpineLash), "no spine while the body is in the way");
    assert!(!acts.contains(&SerpentAct::WormholeGambit));

    // Body gone, head and tail left: the spine is what the segments used to be.
    let acts = serpent_acts(&serpent_parts_with(0, true, true));
    assert_eq!(acts, vec![SerpentAct::SpineLash], "head+tail has exactly one act");

    // Tail gone: the launch goes with it, and the gambit is what remains.
    let acts = serpent_acts(&serpent_parts_with(0, false, true));
    assert_eq!(acts, vec![SerpentAct::WormholeGambit]);
    assert!(!acts.contains(&SerpentAct::TailLaunch), "a destroyed tail cannot launch itself");

    // Nothing left: nothing to do.
    assert!(serpent_acts(&serpent_parts_with(0, false, false)).is_empty());
}

#[test]
fn every_phase_has_something_to_do() {
    // A phase with no available act would leave the serpent prowling forever
    // and the fight unwinnable-looking, which is worse than a hard attack.
    use crate::scenes::game::boss::serpent::serpent_acts;
    for (segs, tail) in [(SERPENT_SEGMENTS, true), (4, true), (1, true), (0, true), (0, false)] {
        assert!(
            !serpent_acts(&serpent_parts_with(segs, tail, true)).is_empty(),
            "no act available at segments={segs} tail={tail}"
        );
    }
}

#[test]
fn the_gambit_gives_a_real_reaction_window() {
    // The capture is only fair because a tether inside the window escapes it.
    // Too short and it is a coin flip; too long and the capture means nothing.
    assert!(
        (15..=45).contains(&SERPENT_GAMBIT_REACT),
        "reaction window is {SERPENT_GAMBIT_REACT} ticks; wanted roughly a quarter to three quarters of a second"
    );
    // The player is delivered far enough out that a tether has somewhere to go.
    assert!(SERPENT_GAMBIT_DROP > PLAYER_R * 8.0);
    // And the dash has to be able to cross that gap inside the window, or the
    // bite would never land and the attack would be theatre.
    let reach = SERPENT_DASH_SPEED * SERPENT_GAMBIT_REACT as f32;
    assert!(
        reach >= SERPENT_GAMBIT_DROP,
        "the head covers {reach:.0}px in the window but is dropped {SERPENT_GAMBIT_DROP:.0}px away"
    );
}

#[test]
fn the_serpent_head_never_outruns_the_player_while_cruising() {
    // The threat is the body sweeping space behind the head, not the head
    // catching you — a head faster than the player's cap makes the whole fight
    // a chase with no counterplay.
    assert!(
        SERPENT_HEAD_SPEED < MOMENTUM_CAP,
        "head cruises at {SERPENT_HEAD_SPEED}, player cap is {MOMENTUM_CAP}"
    );
    // The dash is the deliberate exception, and only lands after a capture.
    assert!(SERPENT_DASH_SPEED > MOMENTUM_CAP);
}

// ── Spine lash choreography ──────────────────────────────────────────────────

#[test]
fn the_lash_takes_up_a_stance_before_it_sweeps() {
    use crate::scenes::game::boss::serpent::serpent_lash_anchors;
    let centre = (0.0_f32, 0.0_f32);
    // Both pieces start somewhere arbitrary on the body's path.
    let from = ((-4000.0_f32, 900.0_f32), (-4600.0_f32, 1100.0_f32));

    // At tick 0 they are still where they were — the move must be a lerp, not
    // a teleport, or the stance reads as the boss blinking across the arena.
    let (h0, t0) = serpent_lash_anchors(0, centre, from);
    assert!((h0.0 - from.0.0).abs() < 1.0 && (h0.1 - from.0.1).abs() < 1.0);
    assert!((t0.0 - from.1.0).abs() < 1.0);

    // By the end of the telegraph they are on OPPOSITE sides of the centre.
    let (h, t) = serpent_lash_anchors(SERPENT_TELEGRAPH_TICKS, centre, from);
    assert!(
        h.0.signum() != t.0.signum(),
        "head at {h:?} and tail at {t:?} are not on opposite sides"
    );
    assert!(h.0.abs() > 1000.0 && t.0.abs() > 1000.0, "both should be well out from centre");
}

#[test]
fn the_lash_pivot_is_off_centre_so_the_safe_side_moves() {
    use crate::scenes::game::boss::serpent::serpent_lash_anchors;
    let centre = (0.0_f32, 0.0_f32);
    let from = ((0.0_f32, 0.0_f32), (0.0_f32, 0.0_f32));

    // Equal radii would make the spine a diameter through the centre: it would
    // pivot about one fixed point and the safe side would be whichever half you
    // started in. Unequal radii make the line translate as it rotates.
    assert!(
        (SERPENT_LASH_TAIL_RATIO - 1.0).abs() > 0.1,
        "the endpoints must orbit at different radii or this is just a pivot"
    );

    // The midpoint of the spine must actually move over the sweep.
    let mid = |ticks: u32| {
        let (h, t) = serpent_lash_anchors(ticks, centre, from);
        ((h.0 + t.0) * 0.5, (h.1 + t.1) * 0.5)
    };
    let a = mid(SERPENT_TELEGRAPH_TICKS);
    let b = mid(SERPENT_TELEGRAPH_TICKS + (SERPENT_LASH_TICKS - SERPENT_TELEGRAPH_TICKS) / 2);
    let moved = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    assert!(moved > 400.0, "the spine's midpoint barely moved ({moved:.0}px) — that is a pivot");
}

#[test]
fn the_sweep_covers_every_angle_exactly_once() {
    use crate::scenes::game::boss::serpent::serpent_lash_anchors;
    let centre = (0.0_f32, 0.0_f32);
    let from = ((0.0_f32, 0.0_f32), (0.0_f32, 0.0_f32));

    // Half a turn takes the line across every angle once. A full turn would
    // repeat ground the player has already cleared and let them wait it out.
    assert!(
        (SERPENT_LASH_ARC - std::f32::consts::PI).abs() < 0.01,
        "the sweep should be a half turn"
    );

    let ang = |ticks: u32| {
        let (h, t) = serpent_lash_anchors(ticks, centre, from);
        (h.1 - t.1).atan2(h.0 - t.0)
    };
    let start = ang(SERPENT_TELEGRAPH_TICKS);
    let end = ang(SERPENT_LASH_TICKS);
    let swept = (end - start).abs();
    assert!(
        swept > 2.0,
        "the spine only swept {swept:.2} rad; it should cross the arena"
    );

    // And it stays inside the play area rather than swinging off the top.
    for step in 0..40 {
        let ticks = SERPENT_TELEGRAPH_TICKS + step * 4;
        let (h, t) = serpent_lash_anchors(ticks, centre, from);
        let worst = h.1.abs().max(t.1.abs());
        assert!(
            worst <= SERPENT_LASH_RADIUS * SERPENT_LASH_Y + 1.0,
            "the sweep reaches {worst:.0}px vertically, past the squashed limit"
        );
    }
}

// ── Serpent fixes ────────────────────────────────────────────────────────────

#[test]
fn the_serpent_stays_out_of_the_death_floor() {
    // The body is tetherable, so a player riding a segment goes wherever the
    // serpent goes. Unclamped it wandered low and took them into the floor —
    // killed by the thing they were correctly using as traversal.
    assert!(SERPENT_Y_MAX < 0.0, "the band must sit above the arena floor");
    // Comfortably above the arena wall's bottom edge (600) with room for the
    // player to hang below a segment they are tethered to.
    assert!(
        SERPENT_Y_MAX < -1000.0,
        "head may reach y={SERPENT_Y_MAX}, too close to the floor for a tethered player"
    );
    assert!(SERPENT_Y_MIN < SERPENT_Y_MAX, "the band must be a band");
    // And the play centre is inside it, or the serpent could never reach the
    // player at all.
    assert!(
        (SERPENT_Y_MIN..=SERPENT_Y_MAX).contains(&BOSS_Y_CENTER),
        "the arena's centre is outside the serpent's band"
    );
}

#[test]
fn the_glow_and_the_hit_box_agree() {
    use crate::scenes::game::boss::serpent::{serpent_seam, serpent_shielded_now};
    // The cue and the damage test read the same threshold, so the armour is
    // visibly open exactly when it can be hurt.
    for c in 1..=SERPENT_SEGMENTS {
        for step in 0..120 {
            let band = step as f32 * 0.1;
            let seam = serpent_seam(c, band);
            let open = seam >= SERPENT_OPEN_AT;
            if open {
                assert!(
                    !serpent_shielded_now(c, band),
                    "segment {c} reads as open while its shield is still up"
                );
            }
        }
    }
    // The threshold has to be near the top of the ramp, or the glow lights
    // while the plating is still visibly shut.
    assert!(SERPENT_OPEN_AT >= 0.8, "open threshold {SERPENT_OPEN_AT} is too early");
    assert!(SERPENT_OPEN_AT <= 1.0);
}

#[test]
fn the_coil_goal_is_something_the_head_can_chase() {
    // Authored at a fixed 0.02 rad/tick the goal ran the ring at ~52px/tick
    // against a head that travels 22, so it could never be caught and the coil
    // read as meandering rather than as a closing ring.
    let rate = SERPENT_HEAD_SPEED / SERPENT_COIL_RADIUS;
    let goal_speed = rate * SERPENT_COIL_RADIUS;
    assert!(
        goal_speed <= SERPENT_HEAD_SPEED + 0.01,
        "the coil goal moves at {goal_speed:.1}px/tick, faster than the head's {SERPENT_HEAD_SPEED}"
    );
    // And the ring must actually close over the act.
    assert!(SERPENT_COIL_CLOSE < SERPENT_COIL_RADIUS);
}

#[test]
fn the_grace_period_is_long_enough_to_read_the_boss() {
    assert!(
        (120..=360).contains(&SERPENT_NOTICE_TICKS),
        "grace is {SERPENT_NOTICE_TICKS} ticks; wanted roughly two to six seconds"
    );
    // It must end before the first attack could otherwise have fired, or the
    // grace would simply be dead time appended to the fight.
    assert!(SERPENT_NOTICE_TICKS <= SERPENT_ATTACK_GAP);
}

#[test]
fn the_tail_sweep_is_answered_by_elevation() {
    use crate::scenes::game::boss::serpent::serpent_acts;
    // It is the only attack in the fight dodged by being higher, so it has to
    // exist wherever the tail does.
    let parts = boss_parts_for_kind(BossKind::Serpent);
    let acts = serpent_acts(&parts);
    assert!(acts.contains(&SerpentAct::TailSweep), "a live tail should be able to sweep");

    // The arc has to cross a meaningful span or it is a poke, not a sweep.
    assert!(SERPENT_SWEEP_ARC > 1.5, "sweep arc {SERPENT_SWEEP_ARC} rad is too narrow");
    // And its reach must cover a good part of the play band.
    assert!(SERPENT_SWEEP_RADIUS > 1500.0);
}

#[test]
fn the_coil_leaves_gaps_a_player_can_fit_through() {
    // The coil's counterplay IS the gaps between segments, so the body must not
    // be a solid wall. Overlapping discs look better but make the signature
    // attack unavoidable.
    let gap = SERPENT_SEGMENT_SPACING - SERPENT_SEGMENT_SIZE;
    assert!(gap > PLAYER_R * 2.0, "gap is {gap}px against a player {}px across", PLAYER_R * 2.0);
    // But not so wide that the body stops reading as one creature.
    assert!(
        SERPENT_SEGMENT_SPACING < SERPENT_SEGMENT_SIZE * 2.0,
        "segments this far apart read as a string of beads, not a serpent"
    );
    // And the whole body still has to fit inside the coil it forms.
    let body = (SERPENT_SEGMENTS + 1) as f32 * SERPENT_SEGMENT_SPACING;
    let ring = std::f32::consts::TAU * SERPENT_COIL_CLOSE;
    assert!(body < ring, "the body ({body:.0}px) is longer than the closed ring ({ring:.0}px)");
}


#[test]
fn boss_order_rows_line_up_with_their_click_targets() {
    use crate::menu::boss_order_row;
    // Font sizes are authored in logical px and scaled to virtual units, while
    // object positions are authored in virtual units. Mixing the two put the
    // click targets 116 units apart under rows only ~30 apart.
    let (y0, h) = boss_order_row(0);
    let (y1, _) = boss_order_row(1);
    assert!(h > 0.0, "a row must have height");
    assert!(y1 > y0 + h, "rows must not overlap");
    assert!(y1 - y0 - h < h * 0.5, "the gap between rows should be small next to the row");
    // Every row is reachable and they run down the panel in order.
    for i in 1..BOSS_ROSTER.len() {
        assert!(boss_order_row(i).0 > boss_order_row(i - 1).0);
    }
}

#[test]
fn the_body_starts_inside_the_rift_it_emerges_from() {
    use crate::scenes::game::boss::serpent::serpent_trail_point;
    // Collapsing the trail onto the hole is what makes pieces come out one at a
    // time. Laid backward from it, each piece sits at its own chain distance
    // BEHIND the portal — outside the swallow radius, so the whole body is
    // visible in a line before the portal has produced anything.
    let hole = (1000.0_f32, -2500.0_f32);
    let trail = vec![(hole.0, hole.1, 0.0_f32)];
    let arc = 0.0_f32;

    for chain in 0..=(SERPENT_SEGMENTS + 1) {
        let behind = chain as f32 * SERPENT_SEGMENT_SPACING;
        let (pos, _) = serpent_trail_point(&trail, arc, behind).expect("fallback point");
        let d = ((pos.0 - hole.0).powi(2) + (pos.1 - hole.1).powi(2)).sqrt();
        assert!(
            d < SERPENT_RIFT_SWALLOW_R,
            "piece {chain} sits {d:.0}px from the rift at emergence — it would be visible outside it"
        );
    }
}

#[test]
fn the_coil_closes_on_a_fixed_point_and_actually_tightens() {
    use crate::scenes::game::boss::serpent::serpent_goal_for;
    let coil_at = (500.0_f32, -2500.0_f32);
    let player = (9999.0_f32, 9999.0_f32); // deliberately elsewhere
    let vel = (0.0_f32, 0.0_f32);

    let r_at = |ticks: u32| {
        let g = serpent_goal_for(SerpentAct::Coil, ticks, player, coil_at, vel);
        ((g.0 - coil_at.0).powi(2) + ((g.1 - coil_at.1) / 0.55).powi(2)).sqrt()
    };
    let start = r_at(0);
    let mid = r_at(SERPENT_COIL_TICKS / 2);
    let end = r_at(SERPENT_COIL_TICKS);
    assert!(start > mid && mid > end, "the coil must tighten: {start:.0} -> {mid:.0} -> {end:.0}");
    assert!((start - SERPENT_COIL_RADIUS).abs() < 20.0);
    assert!((end - SERPENT_COIL_CLOSE).abs() < 20.0);

    // It closes on the captured point, NOT on the live player — a coil that
    // tracks never closes.
    let g = serpent_goal_for(SerpentAct::Coil, SERPENT_COIL_TICKS, player, coil_at, vel);
    assert!(
        (g.0 - player.0).abs() > 1000.0,
        "the coil followed the player instead of closing where they were"
    );

    // And it makes at least a full lap, or it is an arc rather than a coil.
    assert!(SERPENT_COIL_LAPS >= 1.5);
}

#[test]
fn the_head_faces_the_way_it_is_going() {
    use crate::scenes::game::boss::serpent::{serpent_push_trail, serpent_trail_point};
    // The head is sampled newer than the newest trail entry, so its heading has
    // to come from the two most recent samples. Returning 0 left it drawn
    // facing right whatever direction the serpent was travelling.
    for (dx, dy, want) in [
        (4.0_f32, 0.0_f32, 0.0_f32),
        (-4.0, 0.0, std::f32::consts::PI),
        (0.0, 4.0, std::f32::consts::FRAC_PI_2),
        (0.0, -4.0, -std::f32::consts::FRAC_PI_2),
    ] {
        let mut trail: Vec<(f32, f32, f32)> = Vec::new();
        let mut arc = 0.0_f32;
        for i in 0..40 {
            serpent_push_trail(&mut trail, &mut arc, (dx * i as f32, dy * i as f32));
        }
        let (_, deg) = serpent_trail_point(&trail, arc, 0.0).expect("head point");
        let got = deg.to_radians();
        let diff = ((got - want + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU))
            - std::f32::consts::PI;
        assert!(
            diff.abs() < 0.05,
            "moving ({dx},{dy}) the head faces {:.2} rad, wanted {want:.2}", got
        );
    }
}

#[test]
fn the_burrow_visits_every_stage() {
    // The dive has to be a state machine: on a fixed cycle the entry hole
    // stayed where the last emergence was while the head steered away from it,
    // so the serpent vanished and reappeared with no dive on screen.
    // Approach ends on ARRIVAL, which is what guarantees the swim is seen.
    assert!(SERPENT_RIFT_AHEAD > SERPENT_RIFT_SWALLOW_R * 2.0,
        "the portal opens too close to the head for the swim into it to read");

    // Swallowing the whole body must take long enough to watch.
    let swallow_ticks =
        crate::scenes::game::boss::serpent::serpent_body_arc() / SERPENT_RIFT_SWALLOW_SPEED;
    assert!(
        (60.0..400.0).contains(&swallow_ticks),
        "the body takes {swallow_ticks:.0} ticks to go under; wanted one to six seconds"
    );

    // And the act's ceiling has to accommodate every stage of every surfacing,
    // or the sequence is cut off part way through.
    let swim = SERPENT_RIFT_AHEAD / SERPENT_HEAD_SPEED;
    let per = swim + swallow_ticks + SERPENT_RIFT_SURFACE as f32;
    let need = SERPENT_TELEGRAPH_TICKS as f32 + (SERPENT_RIFT_COUNT + 1) as f32 * per;
    assert!(need > 500.0, "the whole burrow should be a set piece");
}

#[test]
fn the_serpent_is_fully_absent_between_portals() {
    // Opening the exit on the same frame the last piece went in made the
    // serpent appear to loop straight back out of the hole it had just entered.
    // The gap is what makes the two portals read as two places.
    assert!(
        SERPENT_RIFT_HIDDEN >= 30,
        "the absence is {SERPENT_RIFT_HIDDEN} ticks — too short to register as gone"
    );
    // But short enough that the fight does not stall with nothing on screen.
    assert!(SERPENT_RIFT_HIDDEN <= 120);

    // Every stage has to fit inside the act's ceiling, or the burrow is cut off
    // part way through.
    let swallow = crate::scenes::game::boss::serpent::serpent_body_arc()
        / SERPENT_RIFT_SWALLOW_SPEED;
    let swim = SERPENT_RIFT_AHEAD / SERPENT_HEAD_SPEED;
    let per = swim + swallow + SERPENT_RIFT_HIDDEN as f32 + SERPENT_RIFT_SURFACE as f32;
    let ceiling = SERPENT_TELEGRAPH_TICKS as f32
        + (SERPENT_RIFT_COUNT + 1) as f32
            * (swim + swallow + SERPENT_RIFT_HIDDEN as f32 + SERPENT_RIFT_SURFACE as f32);
    assert!(
        ceiling >= SERPENT_TELEGRAPH_TICKS as f32 + SERPENT_RIFT_COUNT as f32 * per,
        "the act ends before the last surfacing completes"
    );
}

#[test]
fn a_goal_is_not_a_brake() {
    // Steering the head AT the portal is not the same as stopping it there. It
    // swam straight through, kept going, turned around because the portal was
    // still its goal, and came back — entering twice per cycle with a loop in
    // between. The head has to be frozen outright while it is in the hole.
    //
    // This models the arrival test: the head stops advancing once it is inside,
    // so its distance from the portal can never grow again during the dive.
    let hole = (0.0_f32, 0.0_f32);
    let arrive_at = SERPENT_RIFT_SWALLOW_R * 0.6;

    // Approaching at cruise speed, the head closes on the portal...
    let mut d = SERPENT_RIFT_AHEAD;
    let mut steps = 0;
    while d >= arrive_at && steps < 10_000 {
        d -= SERPENT_HEAD_SPEED;
        steps += 1;
    }
    assert!(steps < 10_000, "the head never reaches the portal");
    assert!(d < arrive_at, "arrival test never fires");
    let _ = hole;

    // ...and from that moment it must not advance, or it exits the far side.
    // The overshoot on the arrival frame has to stay inside the swallow radius,
    // or the head is already visibly out the other side when it "arrives".
    let overshoot = arrive_at - d;
    assert!(
        overshoot < SERPENT_RIFT_SWALLOW_R,
        "the head overshoots {overshoot:.0}px into a {SERPENT_RIFT_SWALLOW_R:.0}px portal"
    );

    // The body must be able to finish entering before anything else happens.
    let swallow = crate::scenes::game::boss::serpent::serpent_body_arc()
        / SERPENT_RIFT_SWALLOW_SPEED;
    assert!(swallow > 30.0, "the body goes under too fast to see ({swallow:.0} ticks)");
}

#[test]
fn a_homing_attack_must_be_slower_than_the_player() {
    // The launched tail homes every frame, so its speed IS its difficulty.
    // Anything near the player's cap cannot be shaken by any manoeuvre, which
    // makes it unavoidable rather than hard.
    assert!(
        SERPENT_TAIL_LAUNCH_SPEED < MOMENTUM_CAP * 0.6,
        "the tail homes at {SERPENT_TAIL_LAUNCH_SPEED} against a player cap of {MOMENTUM_CAP}"
    );
    // But fast enough to still be a threat worth moving away from.
    assert!(SERPENT_TAIL_LAUNCH_SPEED > MOMENTUM_CAP * 0.25);

    // And it must be able to cross a useful distance inside its window, or it
    // never arrives and the attack is theatre.
    let reach = SERPENT_TAIL_LAUNCH_SPEED * SERPENT_TAIL_LAUNCH_TICKS as f32;
    assert!(reach > 2000.0, "the tail only travels {reach:.0}px before it is recalled");
}
