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
fn the_eclipse_lamp_never_blows_out_what_it_lights() {
    // The model, traced:
    //   accum = ambient + color * ndl * intensity * atten
    //   lit   = clamp(base_color * accum, 0, 1)      <- MULTIPLICATIVE
    //   ndl   = 0.4472 constant in 2D
    //   atten = 1 - smoothstep(0, radius, dist), hard cutoff at radius
    //
    // Because `lit` clamps, any accum ABOVE 1 pushes a sprite toward white. The
    // player ball is light-coloured and clips first — driving accum to ~13 to
    // chase an evenly-lit pool turned it into a white disc. 1.0 is a ceiling.
    let accum = |d: f32| {
        let t = (d / ECLIPSE_PLAYER_LIGHT_R).clamp(0.0, 1.0);
        LIGHT_NDL_2D * ECLIPSE_PLAYER_LIGHT_INTENSITY * (1.0 - t * t * (3.0 - 2.0 * t))
    };

    assert!(
        (accum(0.0) - 1.0).abs() < 0.01,
        "the lamp centre sits at accum {:.2}; anything above 1.0 washes the player white",
        accum(0.0)
    );
    // Never exceeds it anywhere, at any distance.
    for i in 0..=100 {
        let d = ECLIPSE_PLAYER_LIGHT_R * (i as f32 / 100.0);
        assert!(accum(d) <= 1.001, "accum {:.2} at {d:.0} px would blow out", accum(d));
    }

    // The trail has to stay clearly lit. `atten` is steepest mid-range, so the
    // lamp is deliberately much wider than the trail to keep the trail in the
    // gentle inner part of the curve.
    let trail = ECLIPSE_LAMP_TRAIL_LEN;
    assert!(
        ECLIPSE_PLAYER_LIGHT_R > trail * 2.0,
        "the lamp is not wide enough to keep the trail off the steep part of the falloff"
    );
    assert!(
        accum(trail) > 0.5,
        "the far end of the trail is only {:.2} lit",
        accum(trail)
    );

    // And darkness still exists on screen: the viewport half-width must land in
    // the dim end of the curve, and nothing is lit past the radius.
    assert!(accum(VW * 0.5) < 0.25, "the screen edges are still {:.2} lit", accum(VW * 0.5));
    assert_eq!(accum(ECLIPSE_PLAYER_LIGHT_R), 0.0, "light leaks past the radius");
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
    assert!(ECLIPSE_NODE_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.2, "markers reach too far");
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
    let ceiling = 0.12;
    assert!(
        ambient_at(mid) < ceiling,
        "still {:.2} ambient at the midpoint — the eclipse is not reading as dark",
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
    assert!(
        (0.02..0.12).contains(&ECLIPSE_MIN_AMBIENT),
        "floor ambient {ECLIPSE_MIN_AMBIENT} is outside the readable band"
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
    for (name, i) in [
        ("player lamp", ECLIPSE_PLAYER_LIGHT_INTENSITY),
        ("node marker", ECLIPSE_NODE_LIGHT_INTENSITY),
        ("well marker", ECLIPSE_GWELL_LIGHT_INTENSITY),
    ] {
        assert!(
            (i * LIGHT_NDL_2D - 1.0).abs() < 0.02,
            "{name} restores to {:.2} of normal, not 1.0",
            i * LIGHT_NDL_2D
        );
    }
    // Markers illuminate themselves; only the lamp lights an area.
    assert!(ECLIPSE_NODE_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.2);
    assert!(ECLIPSE_GWELL_LIGHT_R < ECLIPSE_PLAYER_LIGHT_R * 0.25);
}
