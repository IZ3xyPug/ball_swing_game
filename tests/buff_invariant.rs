//! An expired buff must not still be in force.
//!
//! The buff is two fields: `player_buff` says which one is running and
//! `buff_timer` says how long is left. Every consumer — each boss, the solar
//! shield, the weakpoint checks — used to test `player_buff > 0` alone, while
//! only the tick knew about the timer. So when the buff lapsed it stopped
//! being DRAWN and stayed in force MECHANICALLY: the Colossus went on
//! absorbing hits into an expired buff and the player stopped taking damage.
//!
//! `buff_is_active` is now the single reading of it, behind
//! `State::buff_active()`, so the two fields cannot be believed apart.

use main::state::buff_is_active;

#[test]
fn a_running_buff_is_active() {
    assert!(buff_is_active(1, 300));
    assert!(buff_is_active(1, 1), "one tick left is still one tick");
}

#[test]
fn a_lapsed_timer_means_the_buff_is_not_active() {
    // Exactly the state the bug left behind: the tick stopped drawing the
    // buff and returned without clearing `player_buff`.
    assert!(
        !buff_is_active(1, 0),
        "a buff whose timer has expired must not read as active, however \
         `player_buff` was left"
    );
}

#[test]
fn no_buff_type_means_no_buff_however_much_time_is_on_the_clock() {
    assert!(!buff_is_active(0, 300));
    assert!(!buff_is_active(0, 0));
}
