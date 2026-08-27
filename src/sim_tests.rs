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
    assert_eq!(mins, vec![9, 17, 26, 34, 43, 51, 60], "Normal boss minutes changed");
    assert!(
        (BOSS_INTERVAL_MINUTES - 60.0 / BOSS_ROSTER_SIZE as f32).abs() < 0.01,
        "the advertised interval no longer matches the schedule"
    );
}
