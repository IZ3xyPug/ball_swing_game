use std::sync::{Mutex, OnceLock};

use quartz::SoundHandle;

fn game_bgm_slot() -> &'static Mutex<Option<SoundHandle>> {
    static GAME_BGM: OnceLock<Mutex<Option<SoundHandle>>> = OnceLock::new();
    GAME_BGM.get_or_init(|| Mutex::new(None))
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
