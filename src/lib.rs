use quartz::*;
use ramp::prism;

mod constants;
mod audio_state;
mod images;
mod hud;
mod poisson;
mod state;
mod level_gen;
mod gameplay;
mod objects;
mod menu;
mod scenes;

#[cfg(test)]
mod sim_tests;

use menu::{
    build_gameover_oxygen_scene,
    build_gameover_scene,
    build_gameover_sun_scene,
    build_menu_scene,
};
use scenes::game::build_game_scene;

pub struct App;

impl App {
    fn new(ctx: &mut Context, _assets: Assets) -> impl Drawable {
        let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
        canvas.add_scene(build_menu_scene(ctx));
        canvas.add_scene(build_game_scene(ctx));
        canvas.add_scene(build_gameover_scene(ctx));
        canvas.add_scene(build_gameover_sun_scene(ctx));
        canvas.add_scene(build_gameover_oxygen_scene(ctx));
        canvas.load_scene("menu");
        canvas
    }
}

ramp::run! { |ctx: &mut Context, assets: Assets| { App::new(ctx, assets) } }
