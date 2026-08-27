# Archive

Point-in-time documents kept for history. None of them describe the game as it
is now — read the code or `DESIGN_PLAN.md` instead.

- `FIX_SUMMARY.md`, `VERIFICATION.txt`, `COUNTER_FIX_DOCUMENTATION.md` — three
  documents about a single 2026-08 fix to the game-over counters. That fix
  shipped; the files outlived it.
- `TESTING_INSTRUCTIONS.md` — manual test steps for that same fix.
- `definitive_test.rs` — a standalone `fn main()` that sat at the repo root
  outside `src/`, so nothing ever compiled or ran it. It reproduced the
  game-over counter bug by simulating a `HashMap` of canvas vars.

Live tests are `cargo test --lib` (`src/sim_tests.rs`) and the headless driver
(`cargo run --bin headless`).
