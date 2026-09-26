use std::sync::{Mutex, OnceLock};

use quartz::SoundHandle;

fn game_bgm_slot() -> &'static Mutex<Option<SoundHandle>> {
    static GAME_BGM: OnceLock<Mutex<Option<SoundHandle>>> = OnceLock::new();
    GAME_BGM.get_or_init(|| Mutex::new(None))
}

fn menu_bgm_slot() -> &'static Mutex<Option<SoundHandle>> {
    static MENU_BGM: OnceLock<Mutex<Option<SoundHandle>>> = OnceLock::new();
    MENU_BGM.get_or_init(|| Mutex::new(None))
}

pub fn replace_game_bgm(handle: SoundHandle) {
    if let Ok(mut slot) = game_bgm_slot().lock() {
        if let Some(prev) = slot.take() {
            prev.stop();
        }
        *slot = Some(handle);
    }
}

pub fn stop_game_bgm() {
    if let Ok(mut slot) = game_bgm_slot().lock() {
        if let Some(prev) = slot.take() {
            prev.stop();
        }
    }
}

pub fn has_game_bgm() -> bool {
    game_bgm_slot()
        .lock()
        .ok()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

pub fn replace_menu_bgm(handle: SoundHandle) {
    if let Ok(mut slot) = menu_bgm_slot().lock() {
        if let Some(prev) = slot.take() {
            prev.stop();
        }
        *slot = Some(handle);
    }
}

pub fn stop_menu_bgm() {
    if let Ok(mut slot) = menu_bgm_slot().lock() {
        if let Some(prev) = slot.take() {
            prev.stop();
        }
    }
}

pub fn has_menu_bgm() -> bool {
    menu_bgm_slot()
        .lock()
        .ok()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

pub fn menu_bgm_finished() -> bool {
    menu_bgm_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|h| h.is_finished()))
        .unwrap_or(false)
}

pub fn menu_bgm_playable() -> bool {
    menu_bgm_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|h| h.is_playable()))
        .unwrap_or(false)
}

/// Hold the game's music where it is.
///
/// A boss whose fight is scored against a beat has TWO clocks: the tick loop
/// and the track. `Canvas::pause` stops tick callbacks, so the beat freezes
/// while rodio keeps playing on its own thread — and the two come back out of
/// phase by exactly however long the pause lasted. On the Conductor that moves
/// the scoring window away from the sound the player is timing against, which
/// makes a correct release read as a miss.
pub fn pause_game_bgm() {
    if let Ok(slot) = game_bgm_slot().lock() {
        if let Some(h) = slot.as_ref() { h.pause(); }
    }
}

/// Start it again, in phase with where the tick loop resumes.
pub fn resume_game_bgm() {
    if let Ok(slot) = game_bgm_slot().lock() {
        if let Some(h) = slot.as_ref() { h.resume(); }
    }
}

pub fn set_game_bgm_volume(vol: f32) {
    if let Ok(slot) = game_bgm_slot().lock() {
        if let Some(h) = slot.as_ref() {
            h.set_volume(vol.max(0.0));
        }
    }
}

pub fn set_menu_bgm_volume(vol: f32) {
    if let Ok(slot) = menu_bgm_slot().lock() {
        if let Some(h) = slot.as_ref() {
            h.set_volume(vol.max(0.0));
        }
    }
}
