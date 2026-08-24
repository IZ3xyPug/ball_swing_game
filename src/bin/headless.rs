// ── bin/headless.rs — thin entry point for the window-less sim driver ────────
// Usage: cargo run --bin headless -- --episodes 5 --frames 3600

fn main() {
    let mut episodes: u64 = 5;
    let mut frames: u64 = 3600;
    let mut boss_mode = false;

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
            "--help" | "-h" => {
                println!("headless [--episodes N] [--frames N] [--boss]");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let agg = main::headless::run(episodes, frames, boss_mode);

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
    println!("death_scene_histogram={:?}", agg.death_scene_histogram);
}
