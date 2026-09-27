use quartz::*;
use ramp::prism;
use ramp::Drawable;

mod constants;
mod audio_state;
mod images;
mod hud;
mod poisson;
pub mod state;
mod achievements;
mod difficulty;
mod hazards;
mod mode;
mod level_gen;
mod gameplay;
mod objects;
mod menu;
mod scenes;
mod shop;
mod cosmetics;
mod profile;
pub mod headless;

/// Exposed for the headless binary's report line.
pub fn constants_gen_ahead() -> f32 { constants::GEN_AHEAD }

#[cfg(test)]
mod sim_tests;
#[cfg(test)]
mod render_tests;

use menu::{
    build_profile_scene,
    build_tutorial_scene,
    build_gameover_oxygen_scene,
    build_gameover_scene,
    build_gameover_sun_scene,
    build_menu_scene,
    build_menu_settings_scene,
    build_achievements_scene,
    build_stats_scene,
    build_daily_reward_scene,
    build_boss_order_scene,
};
use scenes::game::build_game_scene;

pub struct App;

impl App {
    fn new(ctx: &mut Context) -> impl Drawable {
        let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
        canvas.add_scene(build_profile_scene(ctx));
        canvas.add_scene(build_tutorial_scene(ctx));
        canvas.add_scene(build_menu_scene(ctx));
        canvas.add_scene(build_game_scene(ctx));
        canvas.add_scene(build_gameover_scene(ctx));
        canvas.add_scene(build_gameover_sun_scene(ctx));
        canvas.add_scene(build_gameover_oxygen_scene(ctx));
        canvas.add_scene(build_menu_settings_scene(ctx));
        canvas.add_scene(build_achievements_scene(ctx));
        canvas.add_scene(build_stats_scene(ctx));
        canvas.add_scene(build_daily_reward_scene(ctx));
        canvas.add_scene(build_boss_order_scene(ctx));
        // Register the menu press handler at app start (not in the menu on_enter)
        // so it's on the live canvas that receives input. In the GUI the menu
        // on_enter registration didn't persist for mouse presses, so this is the
        // reliable point. It guards on is_scene("menu") so it's harmless here.
        // Where a frame's time goes. On Android there is no profiler to reach
        // for, so this is the only way to see it; on desktop it is opt-in
        // through the environment so a normal run stays quiet.
        //
        //   QUARTZ_TICK_PROFILE=1 cargo run
        //   adb logcat | grep 'tick:'
        canvas.set_tick_profiling(
            cfg!(target_os = "android")
                || std::env::var("QUARTZ_TICK_PROFILE").is_ok_and(|v| v != "0"),
        );
        menu::push_menu_press_handler(&mut canvas);
        // Register the game's left-mouse handlers at app start too, so a mouse
        // hold-to-start counts even when the click that navigated into the game
        // scene is still held down.
        scenes::game::events::register_mouse_handlers(&mut canvas);
        // Same reason: the pause menu's and the touch controls' press handlers
        // used to be registered in the game scene's `on_enter`, where a
        // mouse-press handler never receives presses in the live window. That
        // is why the on-screen pause button did nothing and a paused run could
        // not be resumed.
        scenes::game::register_pause_ui_handlers(&mut canvas);
        // Boot straight into a profile when asked. The profiling and headless
        // paths need to reach the menu without a mouse, and the profile scene
        // is mouse-only.
        //
        //   BSG_PROFILE_SLOT=0 QUARTZ_TICK_PROFILE=1 cargo run --release
        match std::env::var("BSG_PROFILE_SLOT").ok().and_then(|v| v.parse::<usize>().ok()) {
            Some(slot) if slot < profile::SLOT_COUNT => {
                canvas.load_scene("profile");
                menu::select_profile_and_continue(&mut canvas, slot);
            }
            _ => canvas.load_scene("profile"),
        }
        canvas
    }
}

ramp::run! { []; |ctx: &mut Context| { App::new(ctx) } }
