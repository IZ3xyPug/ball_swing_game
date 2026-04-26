use super::*;
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
    let mut seed = 42_u64;
    let mut _gen_head_x = SPAWN_X + 800.0;
    let mut _gen_head_y = (HOOK_Y_MIN + HOOK_Y_MAX) * 0.5;
    let batch = gen_hook_batch(&mut seed, SPAWN_X + 800.0, &mut _gen_head_x, &mut _gen_head_y, 60_000.0);

    assert_eq!(batch.len(), MAX_HOOKS_LIVE);

    let mut prev = None::<(f32, f32)>;
    for hook in &batch {
        assert!((HOOK_Y_MIN..=HOOK_Y_MAX).contains(&hook.y), "hook y out of bounds: {}", hook.y);
        if let Some((px, py)) = prev {
            assert!(hook.x > px, "hook x must increase");
            let dist = ((hook.x - px).powi(2) + (hook.y - py).powi(2)).sqrt();
            assert!(dist >= HOOK_MIN_REACH * 0.95, "hooks too close: {dist:.1} px");
            assert!(dist <= HOOK_MAX_REACH * 1.05, "hooks too far: {dist:.1} px");
        }
        prev = Some((hook.x, hook.y));
    }
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

    let g1 = gate_image_cached();
    let g2 = gate_image_cached();
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
        let _ = gate_image_cached();
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_secs_f32() < 2.5, "cache smoke too slow: {elapsed:?}");
}
