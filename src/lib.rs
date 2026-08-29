use quartz::*;
use ramp::prism;
use ramp::Drawable;

mod constants;
mod audio_state;
mod images;
mod hud;
mod poisson;
mod state;
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
mod profile;
pub mod headless;

/// Exposed for the headless binary's report line.
pub fn constants_gen_ahead() -> f32 { constants::GEN_AHEAD }

#[cfg(test)]
mod sim_tests;

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
        // Register the menu press handler at app start (not in the menu on_enter)
        // so it's on the live canvas that receives input. In the GUI the menu
        // on_enter registration didn't persist for mouse presses, so this is the
        // reliable point. It guards on is_scene("menu") so it's harmless here.
        menu::push_menu_press_handler(&mut canvas);
        // Register the game's left-mouse handlers at app start too, so a mouse
        // hold-to-start counts even when the click that navigated into the game
        // scene is still held down.
        scenes::game::events::register_mouse_handlers(&mut canvas);
        canvas.load_scene("profile");
        canvas
    }
}

ramp::run! { []; |ctx: &mut Context| { App::new(ctx) } }
