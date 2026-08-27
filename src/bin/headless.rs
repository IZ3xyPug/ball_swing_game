// ── bin/headless.rs — thin entry point for the window-less sim driver ────────
// Usage: cargo run --bin headless -- --episodes 5 --frames 3600

fn main() {
    let mut episodes: u64 = 5;
    let mut frames: u64 = 3600;
    let mut boss_mode = false;
    let mut fall_test = false;
    let mut boss_warp = false;
    let mut weakpoint_check = false;
    let mut flare_test = false;
    let mut shelter_check = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--episodes" | "-e" => {
                if i + 1 < args.len() {
                    episodes = args[i + 1].parse().unwrap_or(5);
                }
                i += 1;
            }
            "--frames" | "-f" => {
                if i + 1 < args.len() {
                    frames = args[i + 1].parse().unwrap_or(3600);
                }
                i += 1;
            }
            "--boss" | "-b" => {
                boss_mode = true;
            }
            "--boss-warp" => {
                boss_mode = true;
                boss_warp = true;
            }
            "--boss-weakpoint-check" => {
                boss_mode = true;
                boss_warp = true;
                weakpoint_check = true;
            }
            "--flare-test" => {
                flare_test = true;
            }
            "--flare-shelter-check" => {
                shelter_check = true;
            }
            "--fall-test" => {
                fall_test = true;
            }
            "--help" | "-h" => {
                println!("headless [--episodes N] [--frames N] [--boss] [--boss-warp] [--boss-weakpoint-check] [--fall-test] [--flare-test] [--flare-shelter-check]");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let agg = main::headless::run(episodes, frames, boss_mode, fall_test, boss_warp, weakpoint_check, flare_test, shelter_check);

    println!("\n=== HEADLESS SUMMARY ===");
    println!(
        "episodes={} panics={} deaths={} space_entries={} boss_entries={} boss_kills={} max_zone={}",
        agg.episodes, agg.panics, agg.deaths, agg.space_entries, agg.boss_entries, agg.boss_kills, agg.max_zone
    );
    println!(
        "avg_dist={:.0} best_dist={:.0} avg_max_speed={:.1}",
        agg.avg_dist, agg.best_dist, agg.avg_max_speed
    );
    println!(
        "total_frames={} total_hooks_grabbed={} total_coins={}",
        agg.total_frames, agg.total_hooks_grabbed, agg.total_coins
    );
    println!(
        "total_hearts_lost={} final_hearts_sum={} weakpoint_hits={}",
        agg.total_hearts_lost, agg.final_hearts_sum, agg.weakpoint_hits
    );
    println!("death_scene_histogram={:?}", agg.death_scene_histogram);
    let starve_pct = if agg.airborne_frames > 0 {
        100.0 * agg.starved_frames as f64 / agg.airborne_frames as f64
    } else {
        0.0
    };
    println!(
        "starved_frames={}/{} ({:.1}% of airborne)  worst_starve_streak={} frames",
        agg.starved_frames, agg.airborne_frames, starve_pct, agg.worst_starve_streak
    );
    println!(
        "flares_fired={} flare_hearts_lost={} flare_saves={} flares_without_shelter={}",
        agg.flares_fired, agg.flare_hearts_lost, agg.flare_saves, agg.flares_without_shelter
    );
}
