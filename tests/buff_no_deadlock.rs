//! Activating a buff must not deadlock the game tick.
//!
//! `tick_buff` held a `MutexGuard` on the run state and then called
//! `st.lock()` again to set one flag. `std::sync::Mutex` is not reentrant, so
//! the second lock blocked on the guard the same thread was still holding, and
//! the tick never returned. It only fires when a buff actually turns on — rare
//! enough to survive every other test — and the symptom is not a panic: the
//! main thread simply stops. Android reports that as "isn't responding" and
//! kills the app, which reads to a player as a crash on grabbing a buff node.
//!
//! The headless driver's `--boss-weakpoint-check` scenario warps to a boss and
//! forces a buff, so it reproduces this deterministically and with no GPU.
//!
//! Verified to FAIL against the bug: with the second `st.lock()` restored, this
//! scenario ran past 60 seconds without finishing.

use std::sync::mpsc;
use std::time::Duration;

#[test]
fn activating_a_buff_does_not_deadlock_the_tick() {
    let (tx, rx) = mpsc::channel();

    // On a worker thread, so a deadlock fails the test instead of hanging the
    // whole run forever. A detached thread does not keep the process alive.
    std::thread::spawn(move || {
        // episodes, frames, boss_mode, force_fall, boss_warp, weakpoint_check,
        // stasis_down, flare_test, shelter_check, start_minute
        let report = main::headless::run(
            1, 2500, true, false, true, true, false, false, false, 0.0,
        );
        let _ = tx.send(report.panics);
    });

    match rx.recv_timeout(Duration::from_secs(90)) {
        Ok(panics) => assert_eq!(
            panics, 0,
            "the boss/buff scenario panicked {panics} time(s)"
        ),
        Err(_) => panic!(
            "the buff scenario never finished — the tick is deadlocked. \
             Something is taking the run-state mutex while a guard on it is \
             still alive; std::sync::Mutex is not reentrant."
        ),
    }
}
