//! shop_demos.rs — the UPGRADES page's demo clips.
//!
//! Like the ability videos in Just Cause 3: the selected upgrade plays a short
//! looping clip of real gameplay showing what it does — the same run BEFORE
//! and AFTER buying it (a heart joining the bar, the reach ring widening, a
//! fall caught by a free respawn...). The clips are filmed with the headless
//! driver (`HEADLESS_PERM`, `HEADLESS_SHOW_RING`, `HEADLESS_LABEL`) and live
//! in `assets/upgrade_demos/<upgrade id>.gif`.

use quartz::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::constants::*;

/// The panel: where the clip plays (below the SELECT button), and its frame.
pub const DEMO_SIZE: (f32, f32) = (1040.0, 585.0);
pub const DEMO_POS: (f32, f32) = (VW * 0.5 - 520.0, 1545.0);
const DEMO_FPS: f32 = 15.0;

/// The clip for an upgrade id.
fn demo_bytes(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "heart" => include_bytes!("../assets/upgrade_demos/heart.gif"),
        "reach" => include_bytes!("../assets/upgrade_demos/reach.gif"),
        "momentum" => include_bytes!("../assets/upgrade_demos/momentum.gif"),
        "magnet" => include_bytes!("../assets/upgrade_demos/magnet.gif"),
        "purse" => include_bytes!("../assets/upgrade_demos/purse.gif"),
        "flareward" => include_bytes!("../assets/upgrade_demos/flareward.gif"),
        "secondwind" => include_bytes!("../assets/upgrade_demos/secondwind.gif"),
        _ => return None,
    })
}

/// Every clip, decoded once (its frames are shared Arcs, so a clone is cheap).
fn cache() -> &'static Mutex<HashMap<String, Option<AnimatedSprite>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<AnimatedSprite>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn decode(id: &str) -> Option<AnimatedSprite> {
    demo_bytes(id).and_then(|b| AnimatedSprite::new(b, DEMO_SIZE, DEMO_FPS).ok())
}

/// Decode every clip off the game thread. A clip is a megabyte or two of
/// GIF whose every frame is resized to the panel, which took long enough to
/// freeze the carousel the first time each upgrade was scrolled onto.
/// Called when the shop opens; idempotent.
pub fn prewarm() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() { return; }
    let ids: Vec<&'static str> = crate::profile::PERM_UPGRADES.iter().map(|u| u.id).collect();
    let work = move || {
        for id in ids {
            if cache().lock().unwrap().contains_key(id) { continue; }
            let s = decode(id);
            cache().lock().unwrap().insert(id.to_string(), s);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    { std::thread::spawn(work); }
    #[cfg(target_arch = "wasm32")]
    { work(); }
}

/// The clip for `id` if it has been decoded yet; never blocks on a decode.
/// `None` = not ready, `Some(None)` = this upgrade has no clip.
fn demo_sprite(id: &str) -> Option<Option<AnimatedSprite>> {
    prewarm();
    cache().lock().unwrap().get(id).cloned()
}

/// The panel's objects, for `shop::extend_with_shop`.
pub fn demo_objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let pad = 20.0;
    let mut frame = crate::menu_look::glass_obj(ctx, "shop_demo_frame",
        (DEMO_SIZE.0 + pad * 2.0, DEMO_SIZE.1 + pad * 2.0), (DEMO_POS.0 - pad, DEMO_POS.1 - pad),
        crate::menu_look::GLASS_LILAC, true, vec![]);
    frame.visible = false;
    let mut clip = GameObject::new_rect(ctx, "shop_demo".into(), None::<Image>, DEMO_SIZE, DEMO_POS,
        vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    clip.visible = false;
    vec![("shop_demo_frame".into(), frame), ("shop_demo".into(), clip)]
}

/// Show the selected upgrade's clip on the UPGRADES page; hide it elsewhere.
/// Every shop frame.
pub fn tick_demo(c: &mut Canvas) {
    let on = c.get_bool("menu_in_shop") && c.get_i32("shop_screen") == 1
        && crate::shop::is_upgrade_category(c.get_i32("shop_active_category"));
    let idx = c.get_i32("shop_selected").max(0) as usize;
    let id = crate::profile::PERM_UPGRADES.get(idx).map(|u| u.id).unwrap_or("");
    // Still decoding: hide the panel (rather than leave the last upgrade's
    // clip up under this one's name) and look again next frame.
    let ready = if on { demo_sprite(id) } else { Some(None) };
    let Some(sprite) = ready else {
        for n in ["shop_demo_frame", "shop_demo"] {
            if let Some(o) = c.get_game_object_mut(n) { o.visible = false; }
        }
        c.set_var("shop_demo_shown", -2i32);
        return;
    };
    let show = sprite.is_some();
    for n in ["shop_demo_frame", "shop_demo"] {
        if let Some(o) = c.get_game_object_mut(n) { o.visible = show; }
    }
    // A new selection starts its clip from the top.
    let key = if show { idx as i32 } else { -1 };
    if c.get_i32("shop_demo_shown") != key || c.get_var("shop_demo_shown").is_none() {
        c.set_var("shop_demo_shown", key);
        if let (Some(s), Some(o)) = (sprite, c.get_game_object_mut("shop_demo")) {
            o.set_animation(s);
        }
    }
}
